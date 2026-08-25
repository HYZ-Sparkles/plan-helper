/**
 * 表单输入校验/归一工具：与文案（labels.ts）、后端调用（api.ts）分离的小函数。
 */

/**
 * 把常见日期写法归一为 YYYY-MM-DD：支持 2026-12-20 / 2026/12/20 / 20261220。
 * 不是真实存在的日历日期（含 2026-02-30 这类滚动假日期，用回读比对挡住）返回 null。
 * DatePicker 失焦归一用；值要么合法要么回退，调用方无需再校验。
 */
export function normalizeDateString(s: string): string | null {
  let m = /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$/.exec(s.trim());
  if (!m) m = /^(\d{4})(\d{2})(\d{2})$/.exec(s.trim());
  if (!m) return null;
  const [y, mo, d] = m.slice(1).map(Number);
  const date = new Date(y, mo - 1, d); // 本地时间构造，避免时区偏移
  if (date.getFullYear() !== y || date.getMonth() !== mo - 1 || date.getDate() !== d) {
    return null;
  }
  return `${y}-${String(mo).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
}

/**
 * 百分比数值校验（CONTEXT ProgressGranularity，2026-08-24 验收修订）：
 * 任意正数、上限 100、最多一位小数；min 控制下限（汇报 0.1 / 修正 0）。
 * 一位小数判定用容差 1e-9——JS 浮点 0.1/0.3/0.7 等不可精确表示，
 * 严格等式 `Math.round(v*10) === v*10` 会误拒合法输入（与后端 `valid_percent` 对齐）。
 * 后端另有权威校验，这里只做即时反馈。
 */
export function isValidPercentValue(v: number, min: number): boolean {
  return (
    Number.isFinite(v) &&
    v >= min &&
    v <= 100 &&
    Math.abs(v * 10 - Math.round(v * 10)) < 1e-9
  );
}
