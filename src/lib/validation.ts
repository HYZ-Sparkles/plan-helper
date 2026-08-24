/**
 * 表单输入校验工具：与文案（labels.ts）、后端调用（api.ts）分离的小函数。
 */

/**
 * 严格校验 YYYY-MM-DD 是否为真实存在的日历日期。
 * 用"回读比对"挡住 2026-02-30 这类滚动到 3 月的假日期。
 */
export function isValidDateString(s: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(s)) return false;
  const [y, m, d] = s.split("-").map(Number);
  const date = new Date(y, m - 1, d); // 本地时间构造，避免时区偏移
  return date.getFullYear() === y && date.getMonth() === m - 1 && date.getDate() === d;
}
