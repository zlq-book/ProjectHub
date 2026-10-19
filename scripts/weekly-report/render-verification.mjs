import fs from 'node:fs/promises';
import { FileBlob, SpreadsheetFile } from '@oai/artifact-tool';
const input=process.argv[2]??'.verification/weekly-report/weekly-report.xlsx';
const workbook=await SpreadsheetFile.importXlsx(await FileBlob.load(input));
const blob=await workbook.render({sheetName:'工作周报',range:'A1:F18',scale:1.5,format:'png'});
await fs.writeFile('.verification/weekly-report/export-preview.png',new Uint8Array(await blob.arrayBuffer()));
console.log('Excel 渲染完成');
