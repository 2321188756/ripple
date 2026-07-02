import { useCallback, useEffect, useState } from "react";

interface MentionItem {
  id: string;
  label: string;
}

/**
 * @ 补全 hook：监测输入框中 @ 触发条件，提供知识库列表过滤与键盘导航。
 * @param items 可选的补全条目（知识库列表）
 */
export function useMentionCompletion(items: MentionItem[]) {
  const [showMention, setShowMention] = useState(false);
  const [mentionFilter, setMentionFilter] = useState("");
  const [mentionIdx, setMentionIdx] = useState(0);

  const filtered = items.filter(
    (i) => mentionFilter === "" || i.label.includes(mentionFilter),
  );

  /** 根据光标位置判断是否应弹出补全 */
  const detectMention = (value: string, pos: number) => {
    const before = value.slice(0, pos);
    const at = before.lastIndexOf("@");
    if (at >= 0 && (at === 0 || " \n".includes(before[at - 1]))) {
      setShowMention(true);
      setMentionFilter(before.slice(at + 1).match(/^[\w一-鿿\-]*/)?.[0] || "");
      setMentionIdx(0);
    } else {
      setShowMention(false);
    }
  };

  /** 选中某个补全项，返回新的输入文本 */
  const selectMention = useCallback(
    (label: string, input: string, focusEl: HTMLTextAreaElement | null) => {
      const pos = focusEl?.selectionStart ?? input.length;
      const before = input.slice(0, pos);
      const at = before.lastIndexOf("@");
      if (at >= 0) {
        const after = before.slice(at);
        const len = after.match(/^@[\w一-鿿\-]*/)?.[0].length || 0;
        const newInput = input.slice(0, at) + "@" + label + input.slice(at + len);
        setShowMention(false);
        focusEl?.focus();
        return newInput;
      }
      setShowMention(false);
      return input;
    },
    [],
  );

  /** 键盘导航：返回是否消费了该按键 */
  const handleKeyDown = (e: React.KeyboardEvent): boolean => {
    if (!showMention) return false;
    if (e.key === "Enter") {
      e.preventDefault();
      if (filtered[mentionIdx]) return true; // 由调用方触发 selectMention
      return true;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setMentionIdx((i) => Math.min(i + 1, filtered.length - 1));
      return true;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setMentionIdx((i) => Math.max(i - 1, 0));
      return true;
    }
    if (e.key === "Escape") {
      setShowMention(false);
      return true;
    }
    return false;
  };

  // 补全项变化时重置索引
  useEffect(() => {
    setMentionIdx(0);
  }, [mentionFilter]);

  return {
    showMention,
    filtered,
    mentionIdx,
    detectMention,
    selectMention,
    handleKeyDown,
    hide: () => setShowMention(false),
  };
}
