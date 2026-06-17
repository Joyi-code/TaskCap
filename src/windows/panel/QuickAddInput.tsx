import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MutableRefObject,
  type Ref,
} from "react";
import {
  buildQuickAddMenuItems,
  filterQuickAddMenuItems,
  insertQuickAddToken,
  QuickAddMenuItem,
} from "./quickAddAssist";

type Props = {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  knownTags?: string[];
  knownProjects?: string[];
  onSubmit?: () => void;
  onKeyDownExtra?: (event: React.KeyboardEvent<HTMLInputElement>) => void;
  /** 独立快速新增窗口内菜单向上展开，避免被窗口裁切 */
  menuPlacement?: "above" | "below";
  /** 符号菜单显隐回调（供独立窗口据此调整高度） */
  onMenuToggle?: (open: boolean) => void;
  /** 菜单未展开时按 Esc 触发（独立窗口据此关闭自身） */
  onRequestClose?: () => void;
  /** 外部持有 input 引用，供独立窗口打开时聚焦 */
  inputRef?: Ref<HTMLInputElement | null>;
  /** 挂载后自动聚焦（快速新增窗口） */
  autoFocus?: boolean;
  /** input 节点挂载回调 */
  onInputMount?: (input: HTMLInputElement | null) => void;
};

export function QuickAddInput({
  value,
  onChange,
  placeholder,
  className,
  knownTags = [],
  knownProjects = [],
  onSubmit,
  onKeyDownExtra,
  menuPlacement = "below",
  onMenuToggle,
  onRequestClose,
  inputRef: externalInputRef,
  autoFocus = false,
  onInputMount,
}: Props) {
  const localInputRef = useRef<HTMLInputElement | null>(null);
  const autoFocusDoneRef = useRef(false);
  const setInputRef = useCallback((node: HTMLInputElement | null) => {
    localInputRef.current = node;
    if (typeof externalInputRef === "function") {
      externalInputRef(node);
    } else if (externalInputRef && "current" in externalInputRef) {
      (externalInputRef as MutableRefObject<HTMLInputElement | null>).current = node;
    }
    onInputMount?.(node);
    if (node && autoFocus && !autoFocusDoneRef.current) {
      autoFocusDoneRef.current = true;
      requestAnimationFrame(() => {
        node.focus();
        node.select?.();
      });
    }
  }, [autoFocus, externalInputRef, onInputMount]);
  const [menuOpen, setMenuOpen] = useState(false);
  const [highlightIndex, setHighlightIndex] = useState(0);

  const allItems = useMemo(
    () => buildQuickAddMenuItems(knownTags, knownProjects),
    [knownTags, knownProjects],
  );

  const visibleItems = useMemo(() => {
    const cursor = localInputRef.current?.selectionStart ?? value.length;
    return filterQuickAddMenuItems(allItems, value, cursor);
  }, [allItems, value, menuOpen]);

  useEffect(() => {
    if (!menuOpen) return;
    setHighlightIndex((idx) => Math.min(idx, Math.max(visibleItems.length - 1, 0)));
  }, [menuOpen, visibleItems.length]);

  const menuVisible = menuOpen && visibleItems.length > 0;

  // 菜单显隐变化时通知父级（独立窗口据此增减高度）
  useEffect(() => {
    onMenuToggle?.(menuVisible);
  }, [menuVisible, onMenuToggle]);

  function closeMenu() {
    setMenuOpen(false);
    setHighlightIndex(0);
  }

  function applyItem(item: QuickAddMenuItem) {
    const input = localInputRef.current;
    if (!input) return;
    const start = input.selectionStart ?? value.length;
    const end = input.selectionEnd ?? value.length;
    const next = insertQuickAddToken(value, item.insert, start, end);
    onChange(next.value);
    closeMenu();
    requestAnimationFrame(() => {
      input.focus();
      input.setSelectionRange(next.cursor, next.cursor);
      // 仅插入标准符号时保持菜单打开，方便继续选子项
      if (item.insert === "#" || item.insert === "!" || item.insert === "/" || item.insert === "+") {
        setMenuOpen(true);
        setHighlightIndex(0);
      }
    });
  }

  function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
    const nextValue = event.target.value;
    const cursor = event.target.selectionStart ?? nextValue.length;
    onChange(nextValue);
    const before = nextValue.slice(0, cursor);
    if (/(?:#|!|\+)[\p{L}\p{N}_-]*$/u.test(before) || /\/\d{0,3}$/u.test(before)) {
      setMenuOpen(true);
      setHighlightIndex(0);
    } else if (menuOpen) {
      closeMenu();
    }
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    onKeyDownExtra?.(event);
    if (event.defaultPrevented) return;

    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      if (menuOpen && visibleItems.length > 0) {
        applyItem(visibleItems[highlightIndex]);
        return;
      }
      onSubmit?.();
      return;
    }

    if (event.key === "Escape") {
      if (menuOpen) {
        event.preventDefault();
        closeMenu();
        return;
      }
      if (onRequestClose) {
        event.preventDefault();
        onRequestClose();
      }
      return;
    }

    if (event.key === "ArrowDown" && menuOpen) {
      event.preventDefault();
      setHighlightIndex((idx) => (idx + 1) % visibleItems.length);
      return;
    }

    if (event.key === "ArrowUp" && menuOpen) {
      event.preventDefault();
      setHighlightIndex((idx) => (idx - 1 + visibleItems.length) % visibleItems.length);
      return;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      if (!menuOpen) {
        setMenuOpen(true);
        setHighlightIndex(0);
        return;
      }
      if (visibleItems.length > 0) {
        applyItem(visibleItems[highlightIndex]);
      }
    }
  }

  return (
    <div
      className={`quick-add-input-wrap${menuPlacement === "above" ? " menu-above" : ""}`}
    >
      <input
        ref={setInputRef}
        className={className}
        placeholder={placeholder}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onBlur={() => {
          // 延迟关闭，便于点击菜单项
          window.setTimeout(() => closeMenu(), 120);
        }}
        autoComplete="off"
        spellCheck={false}
      />
      {menuVisible ? (
        <div className="quick-add-token-menu" role="listbox" aria-label="快速插入标签、优先级或预计时长">
          {visibleItems.map((item, index) => (
            <button
              key={item.id}
              type="button"
              role="option"
              aria-selected={index === highlightIndex}
              className={`quick-add-token-item${index === highlightIndex ? " is-active" : ""}`}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => applyItem(item)}
            >
              <span className="quick-add-token-symbol">{item.label}</span>
              {item.hint ? <span className="quick-add-token-hint">{item.hint}</span> : null}
            </button>
          ))}
          <div className="quick-add-token-footer">Tab 选择 · ↑↓ 移动 · Esc 关闭</div>
        </div>
      ) : null}
    </div>
  );
}
