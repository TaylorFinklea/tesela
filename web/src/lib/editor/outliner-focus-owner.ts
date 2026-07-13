type FocusRoot = Pick<Node, "contains">;
type FocusDocument = Pick<Document, "activeElement">;

export function outlinerOwnsDocumentFocus(
  root: FocusRoot | null | undefined,
  ownerDocument: FocusDocument | null = typeof document !== "undefined" ? document : null,
): boolean {
  const activeElement = ownerDocument?.activeElement;
  return !!root && !!activeElement && root.contains(activeElement);
}
