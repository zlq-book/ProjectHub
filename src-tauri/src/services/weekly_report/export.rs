use super::protocol::ReportContent;
use std::collections::HashMap;
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn title(c: &ReportContent) -> String {
    use chrono::Datelike;
    let start = chrono::NaiveDate::parse_from_str(&c.range.start_date, "%Y-%m-%d");
    let end = chrono::NaiveDate::parse_from_str(&c.range.end_date, "%Y-%m-%d");
    match (start, end) {
        (Ok(a), Ok(b)) => format!(
            "{}年工作周报（{}月{}日——{}{}月{}日）",
            a.year(),
            a.month(),
            a.day(),
            if a.year() != b.year() {
                format!("{}年", b.year())
            } else {
                String::new()
            },
            b.month(),
            b.day()
        ),
        _ => "工作周报".into(),
    }
}
fn outcomes(c: &ReportContent) -> Vec<&str> {
    c.selected_outcome_ids
        .iter()
        .filter_map(|id| {
            c.outcomes
                .iter()
                .find(|o| &o.id == id)
                .map(|o| o.text.as_str())
        })
        .collect()
}
pub fn markdown(c: &ReportContent) -> Result<String, String> {
    c.validate(true)?;
    let safe = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\\', "\\\\")
            .replace('*', "\\*")
            .replace('_', "\\_")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('#', "\\#")
    };
    let mut text = format!(
        "# {}\n\n所属部门：{}\n\n填报人：{}\n",
        title(c),
        safe(&c.department),
        safe(&c.reporter)
    );
    for (name, items) in [
        ("主要完成工作", outcomes(c)),
        (
            "困难/需要协调解决的问题",
            c.difficulties.iter().map(String::as_str).collect(),
        ),
        (
            "下周主要工作计划",
            c.plans.iter().map(String::as_str).collect(),
        ),
    ] {
        text.push_str(&format!("\n## {name}\n\n"));
        for (i, s) in items.iter().enumerate() {
            text.push_str(&format!("{}. {}\n", i + 1, safe(s).replace('\n', "\n   ")));
        }
    }
    text.push_str(&format!(
        "\n## 本周感悟/在工作中提炼的经验、知识和技能\n\n{}\n",
        safe(&c.reflection)
    ));
    Ok(text)
}
// 只打包应用内置且已验证的固定资源；不解压外部 ZIP，不支持通用 ZIP 输入。
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for b in bytes {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    !crc
}
fn u16le(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_le_bytes())
}
fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes())
}
pub fn package(entries: Vec<(&str, Vec<u8>)>) -> Result<Vec<u8>, String> {
    let mut out = vec![];
    let mut central = vec![];
    let count = entries.len();
    if count > 65535 {
        return Err("模板文件过多".into());
    }
    for (name, data) in entries {
        if data.len() > 8 * 1024 * 1024 || name.len() > 65535 {
            return Err("模板资源过大".into());
        }
        let offset = out.len() as u32;
        let crc = crc32(&data);
        u32le(&mut out, 0x04034b50);
        for v in [20, 0x800, 0, 0, 33] {
            u16le(&mut out, v)
        }
        for v in [crc, data.len() as u32, data.len() as u32] {
            u32le(&mut out, v)
        }
        u16le(&mut out, name.len() as u16);
        u16le(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(&data);
        u32le(&mut central, 0x02014b50);
        for v in [20, 20, 0x800, 0, 0, 33] {
            u16le(&mut central, v)
        }
        for v in [crc, data.len() as u32, data.len() as u32] {
            u32le(&mut central, v)
        }
        for v in [name.len() as u16, 0, 0, 0, 0] {
            u16le(&mut central, v)
        }
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend(name.as_bytes());
    }
    let start = out.len() as u32;
    let size = central.len() as u32;
    out.extend(central);
    u32le(&mut out, 0x06054b50);
    for v in [0, 0, count as u16, count as u16] {
        u16le(&mut out, v)
    }
    u32le(&mut out, size);
    u32le(&mut out, start);
    u16le(&mut out, 0);
    Ok(out)
}
pub fn xlsx(c: &ReportContent) -> Result<Vec<u8>, String> {
    c.validate(true)?;
    let mut values = HashMap::new();
    values.insert("A1".to_string(), title(c));
    values.insert("A2".into(), format!("所属部门：{}", c.department));
    values.insert("A3".into(), format!("填报人：{}", c.reporter));
    values.insert("B16".into(), c.reflection.clone());
    for (start, max, items) in [
        (5, 4, outcomes(c)),
        (9, 3, c.difficulties.iter().map(String::as_str).collect()),
        (12, 4, c.plans.iter().map(String::as_str).collect()),
    ] {
        for i in 0..max {
            values.insert(
                format!("C{}", start + i),
                items.get(i).copied().unwrap_or("").to_string(),
            );
        }
    }
    if values.values().any(|s| {
        s.chars()
            .any(|c| c < ' ' && !matches!(c, '\n' | '\r' | '\t'))
    }) {
        return Err("正文含 Excel 不支持的控制字符，请删除后导出".into());
    }
    let xml = include_str!("../../../resources/weekly-report/xl/worksheets/sheet1.xml");
    let doc = roxmltree::Document::parse(xml).map_err(|_| "内置模板无效")?;
    let mut edits = vec![];
    for cell in doc.descendants().filter(|n| n.has_tag_name("c")) {
        if let Some(value) = cell.attribute("r").and_then(|r| values.get(r)) {
            let address = cell.attribute("r").unwrap();
            let style = cell.attribute("s").unwrap_or("0");
            edits.push((cell.range(),format!("<x:c r=\"{address}\" s=\"{style}\" t=\"inlineStr\"><x:is><x:t xml:space=\"preserve\">{}</x:t></x:is></x:c>",escape(value))));
        }
    }
    for row in doc.descendants().filter(|n| n.has_tag_name("row")) {
        let r = row
            .attribute("r")
            .unwrap_or("")
            .parse::<usize>()
            .unwrap_or(0);
        let (text, width) = if (5..=15).contains(&r) {
            (
                values
                    .get(&format!("C{r}"))
                    .map(String::as_str)
                    .unwrap_or(""),
                65,
            )
        } else if r == 16 {
            (c.reflection.as_str(), 75)
        } else {
            continue;
        };
        let lines = text
            .split('\n')
            .map(|s| {
                let units: usize = s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
                (units.max(1) + width - 1) / width
            })
            .sum::<usize>();
        let height = if r == 16 {
            ((lines as f64 * 16.0 + 18.0) / 3.0).max(27.0)
        } else {
            (lines as f64 * 16.0 + 10.0).max(27.0)
        };
        let actual = if r == 16 {
            height.min(409.0)
        } else {
            height.min(409.0)
        };
        if height > 409.0 {
            return Err("正文过长，固定表格无法完整显示，请精简该条后导出".into());
        }
        let begin = row.range().start;
        let end = begin + xml[begin..].find('>').ok_or("模板行无效")? + 1;
        let attrs = row
            .attributes()
            .filter(|a| a.name() != "ht" && a.name() != "customHeight")
            .map(|a| format!(" {}=\"{}\"", a.name(), escape(a.value())))
            .collect::<String>();
        edits.push((
            begin..end,
            format!("<x:row{attrs} ht=\"{actual}\" customHeight=\"1\">"),
        ));
        if r == 16 {
            for tail in [17, 18] {
                if let Some(n) = doc.descendants().find(|n| {
                    n.has_tag_name("row") && n.attribute("r") == Some(tail.to_string().as_str())
                }) {
                    let b = n.range().start;
                    let e = b + xml[b..].find('>').unwrap() + 1;
                    edits.push((
                        b..e,
                        format!("<x:row r=\"{tail}\" ht=\"{actual}\" customHeight=\"1\">"),
                    ));
                }
            }
        }
    }
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut sheet = xml.to_string();
    for (range, text) in edits {
        sheet.replace_range(range, &text)
    }
    let workbook = include_str!("../../../resources/weekly-report/xl/workbook.xml");
    let mut book = workbook.to_string();
    let pos = book.rfind("</").ok_or("模板工作簿无效")?;
    book.insert_str(pos,"<x:definedNames><x:definedName name=\"_xlnm.Print_Area\" localSheetId=\"0\">'工作周报'!$A$1:$F$18</x:definedName></x:definedNames>");
    let entries = vec![
        ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
        ("xl/workbook.xml", book.into_bytes()),
        (
            "xl/styles.xml",
            include_bytes!("../../../resources/weekly-report/xl/styles.xml").to_vec(),
        ),
        (
            "xl/theme/theme1.xml",
            include_bytes!("../../../resources/weekly-report/xl/theme/theme1.xml").to_vec(),
        ),
        (
            "xl/sharedStrings.xml",
            include_bytes!("../../../resources/weekly-report/xl/sharedStrings.xml").to_vec(),
        ),
        (
            "_rels/.rels",
            include_bytes!("../../../resources/weekly-report/_rels/.rels").to_vec(),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            include_bytes!("../../../resources/weekly-report/xl/_rels/workbook.xml.rels").to_vec(),
        ),
        (
            "[Content_Types].xml",
            include_bytes!("../../../resources/weekly-report/[Content_Types].xml").to_vec(),
        ),
    ];
    package(entries)
}
pub fn filename(c: &ReportContent, extension: &str) -> String {
    let reporter = c
        .reporter
        .chars()
        .map(|c| {
            if c < ' ' || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect::<String>();
    format!(
        "{}-{}工作周报-{}.{}",
        c.range.start_date,
        c.range.end_date,
        reporter.trim_end_matches([' ', '.']),
        extension
    )
}
