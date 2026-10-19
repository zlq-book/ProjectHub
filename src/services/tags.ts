// Unicode White_Space 与 BOM 的并集，与 Rust 标签服务保持一致。
export function trimTagName(value: string) { return value.replace(/^[\s\u0085]+|[\s\u0085]+$/gu, ''); }
