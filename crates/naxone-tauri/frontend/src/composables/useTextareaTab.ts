/**
 * 共享 textarea Tab 处理：按 Tab 插入制表符而不是切换焦点。
 * 用法：<textarea spellcheck="false" @keydown="onTextareaTab" />
 *
 * 用 execCommand("insertText") 而非 ta.value=...，是为了保留浏览器原生的
 * 撤销栈（直接赋值 value 会清空 undo 历史，导致 Ctrl+Z 失效）。
 */
export function onTextareaTab(e: KeyboardEvent) {
  if (e.key !== "Tab" || e.shiftKey) return;
  e.preventDefault();
  const ta = e.target as HTMLTextAreaElement;
  ta.focus();
  document.execCommand("insertText", false, "\t");
}
