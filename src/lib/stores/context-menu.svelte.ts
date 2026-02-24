export interface ContextMenuItem {
  label: string;
  action: () => void;
  icon?: string;
  disabled?: boolean;
  separator?: boolean;
}

let visible = $state(false);
let x = $state(0);
let y = $state(0);
let items = $state<ContextMenuItem[]>([]);

export function getContextMenuStore() {
  return {
    get visible() { return visible; },
    get x() { return x; },
    get y() { return y; },
    get items() { return items; },
    show(posX: number, posY: number, menuItems: ContextMenuItem[]) {
      x = posX;
      y = posY;
      items = menuItems;
      visible = true;
    },
    hide() {
      visible = false;
      items = [];
    },
  };
}
