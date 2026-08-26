export function splitLines(source) {
  if (source === null) return [];
  return source.replace(/\n$/, "").split("\n");
}

function buildLineMatchTable(left, right) {
  const table = Array.from({ length: left.length + 1 }, () => Array(right.length + 1).fill(0));
  for (let i = left.length - 1; i >= 0; i--) for (let j = right.length - 1; j >= 0; j--) table[i][j] = left[i] === right[j] ? table[i + 1][j + 1] + 1 : Math.max(table[i + 1][j], table[i][j + 1]);
  return table;
}

function collectDiffRows(left, right, table) {
  const rows = [];
  let i = 0; let j = 0;
  while (i < left.length || j < right.length) {
    if (i < left.length && j < right.length && left[i] === right[j]) rows.push({ kind: "context", text: left[i++], oldLine: i, newLine: ++j });
    else if (j < right.length && (i === left.length || table[i][j + 1] >= table[i + 1][j])) rows.push({ kind: "addition", text: right[j++], oldLine: null, newLine: j });
    else rows.push({ kind: "deletion", text: left[i++], oldLine: i, newLine: null });
  }
  return rows;
}

export function lineDiff(before, after) {
  const left = splitLines(before);
  const right = splitLines(after);
  return collectDiffRows(left, right, buildLineMatchTable(left, right));
}
