export type QuickAddTokenKind = "tag" | "priority" | "duration" | "project";

export type QuickAddMenuItem = {
  id: string;
  kind: QuickAddTokenKind;
  label: string;
  insert: string;
  hint?: string;
};

const MAX_TAG_SUGGESTIONS = 10;

/** Tab 补全菜单：符号入口 + 已有标签/项目 + 优先级/时长快捷项 */
export function buildQuickAddMenuItems(
  knownTags: string[],
  knownProjects: string[],
): QuickAddMenuItem[] {
  const items: QuickAddMenuItem[] = [
    { id: "sym-tag", kind: "tag", label: "#", insert: "#", hint: "添加标签" },
    { id: "sym-priority", kind: "priority", label: "!", insert: "!", hint: "设置优先级" },
    { id: "sym-duration", kind: "duration", label: "/", insert: "/", hint: "预计时长" },
    { id: "pri-high", kind: "priority", label: "!高", insert: "!高", hint: "高优先级" },
    { id: "pri-medium", kind: "priority", label: "!中", insert: "!中", hint: "中优先级" },
    { id: "pri-low", kind: "priority", label: "!低", insert: "!低", hint: "低优先级" },
    { id: "dur-15", kind: "duration", label: "/15m", insert: "/15m", hint: "预计 15 分钟" },
    { id: "dur-25", kind: "duration", label: "/25m", insert: "/25m", hint: "预计 25 分钟" },
    { id: "dur-30", kind: "duration", label: "/30m", insert: "/30m", hint: "预计 30 分钟" },
    { id: "dur-45", kind: "duration", label: "/45m", insert: "/45m", hint: "预计 45 分钟" },
    { id: "dur-60", kind: "duration", label: "/60m", insert: "/60m", hint: "预计 60 分钟" },
    { id: "dur-90", kind: "duration", label: "/90m", insert: "/90m", hint: "预计 90 分钟" },
    { id: "dur-120", kind: "duration", label: "/120m", insert: "/120m", hint: "预计 120 分钟" },
  ];

  for (const tag of knownTags) {
    items.push({
      id: `tag-${tag}`,
      kind: "tag",
      label: `#${tag}`,
      insert: `#${tag}`,
      hint: "已有标签",
    });
  }

  items.push({
    id: "sym-project",
    kind: "project",
    label: "+",
    insert: "+",
    hint: "添加项目",
  });

  for (const project of knownProjects) {
    items.push({
      id: `project-${project}`,
      kind: "project",
      label: `+${project}`,
      insert: `+${project}`,
      hint: "已有项目",
    });
  }

  return items;
}

/** 根据当前输入与光标，过滤 Tab 菜单项 */
export function filterQuickAddMenuItems(
  items: QuickAddMenuItem[],
  value: string,
  cursor: number,
): QuickAddMenuItem[] {
  const before = value.slice(0, cursor);
  const hashMatch = before.match(/#([\p{L}\p{N}_-]*)$/u);
  if (hashMatch) {
    const prefix = hashMatch[1] ?? "";
    const tagItems = items.filter((item) => item.kind === "tag" && item.id !== "sym-tag");
    const filtered = tagItems.filter((item) => {
      const name = item.insert.slice(1);
      return name.toLowerCase().includes(prefix.toLowerCase());
    }).slice(0, MAX_TAG_SUGGESTIONS);
    return filtered.length > 0 ? filtered : [{ id: "sym-tag", kind: "tag", label: "#", insert: "#", hint: "继续输入标签名" }];
  }

  const bangMatch = before.match(/!([\p{L}\p{N}_-]*)$/iu);
  if (bangMatch) {
    const prefix = (bangMatch[1] ?? "").toLowerCase();
    const priItems = items.filter((item) => item.kind === "priority" && item.id.startsWith("pri-"));
    const filtered = priItems.filter((item) => item.insert.slice(1).toLowerCase().startsWith(prefix));
    return filtered.length > 0
      ? filtered
      : [
          { id: "sym-priority", kind: "priority", label: "!", insert: "!", hint: "高/中/低" },
          ...priItems,
        ];
  }

  const slashMatch = before.match(/\/(\d{0,3})$/u);
  if (slashMatch) {
    const prefix = slashMatch[1] ?? "";
    const durationItems = items.filter((item) => item.kind === "duration" && item.id.startsWith("dur-"));
    const filtered = durationItems.filter((item) => item.insert.slice(1).startsWith(prefix));
    return filtered.length > 0
      ? filtered
      : [{ id: "sym-duration", kind: "duration", label: "/", insert: "/", hint: "继续输入预计时长" }];
  }

  const plusMatch = before.match(/\+([\p{L}\p{N}_-]*)$/u);
  if (plusMatch) {
    const prefix = plusMatch[1] ?? "";
    const projectItems = items.filter((item) => item.kind === "project" && item.id !== "sym-project");
    const filtered = projectItems.filter((item) => {
      const name = item.insert.slice(1);
      return name.toLowerCase().startsWith(prefix.toLowerCase());
    });
    return filtered.length > 0
      ? filtered
      : [{ id: "sym-project", kind: "project", label: "+", insert: "+", hint: "继续输入项目名" }];
  }

  // 默认固定展示 #、!、/ 三个标准符号入口，再展示常用快捷项
  const coreItems = items.filter(
    (item) =>
      item.id === "sym-tag" ||
      item.id === "sym-priority" ||
      item.id === "sym-duration" ||
      item.id.startsWith("pri-") ||
      item.id.startsWith("dur-"),
  );
  const tagItems = items
    .filter((item) => item.kind === "tag" && item.id.startsWith("tag-"))
    .slice(0, MAX_TAG_SUGGESTIONS);
  const projectItems = items.filter((item) => item.kind === "project" && item.id.startsWith("project-"));
  return [...coreItems, ...tagItems, ...projectItems];
}

export function insertQuickAddToken(
  value: string,
  insert: string,
  selectionStart: number,
  selectionEnd: number,
): { value: string; cursor: number } {
  const before = value.slice(0, selectionStart);
  const after = value.slice(selectionEnd);

  // 若光标前正在输入未完成的 #tag / !pri / /time / +proj，替换该片段
  const replaceMatch = before.match(/(?:#|!|\/|\+)([\p{L}\p{N}_-]*)$/u);
  const baseBefore = replaceMatch ? before.slice(0, -replaceMatch[0].length) : before;

  const needsSpaceBefore = baseBefore.length > 0 && !/\s$/.test(baseBefore);
  const token = `${needsSpaceBefore ? " " : ""}${insert}`;
  const nextValue = `${baseBefore}${token}${after}`;
  const cursor = baseBefore.length + token.length;
  return { value: nextValue, cursor };
}
