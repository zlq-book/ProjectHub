"""独立读取 XLSX，核对内置模板与生产导出的固定契约。"""
import sys
from pathlib import Path
import zipfile
import openpyxl

path = Path(sys.argv[1])
book = openpyxl.load_workbook(path, data_only=False)
assert book.sheetnames == ['工作周报']
sheet = book.active
assert sheet['A4'].value == '工作类别'
assert sheet['B4'].value == '序号'
assert sheet['C4'].value == '具体事项'
assert {'A1:F1','A5:A8','A9:A11','A12:A15','A16:A18','B16:F18'} <= {str(v) for v in sheet.merged_cells.ranges}
assert sheet['A1'].font.sz == 20
assert sheet['C5'].alignment.wrap_text
assert not any(cell.data_type == 'f' for row in sheet for cell in row)
with zipfile.ZipFile(path) as archive:
    assert archive.testzip() is None
    assert not any('<姓名>' in archive.read(name).decode('utf-8', 'ignore') for name in archive.namelist())
if len(sys.argv) > 2 and sys.argv[2] == '--export':
    assert sheet['A1'].value == '2026年工作周报（10月12日——10月16日）'
    assert sheet['C5'].value == '=1+1 <接口>&测试 😀\n第二行'
    assert sheet['C5'].data_type == 's'
    assert sheet['A2'].value == '所属部门：信息技术部'
    assert sheet['A3'].value == '填报人：测试人员'
    assert sheet['B16'].value == '通过接口联调梳理异常处理。' * 20
    assert sheet.row_dimensions[16].height > 27
    assert '$A$1:$F$18' in str(sheet.print_area)
print('模板、文本类型、合并区域、内容及 ZIP 完整性验证通过')
