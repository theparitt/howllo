import type { Tag as TagType } from "@howllo/types";

export function Tag({ tag }: { tag: TagType }) {
  return (
    <span
      className="howllo-tag"
      style={tag.color ? { backgroundColor: tag.color } : undefined}
    >
      {tag.name}
    </span>
  );
}
