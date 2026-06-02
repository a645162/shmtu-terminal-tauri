import { useEffect } from 'react';

function shouldAllowNativeContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return true;
  }

  if (target.closest('[data-native-context-menu="true"]')) {
    return true;
  }

  const tagName = target.tagName.toLowerCase();
  if (tagName === 'input' || tagName === 'textarea' || tagName === 'select') {
    return true;
  }

  if (target.isContentEditable || target.closest('[contenteditable="true"]')) {
    return true;
  }

  const selection = window.getSelection();
  if (selection && selection.toString().trim()) {
    return true;
  }

  return false;
}

export const GlobalContextMenuGuard = () => {
  useEffect(() => {
    const handleContextMenu = (event: MouseEvent) => {
      const target = event.target;

      if (shouldAllowNativeContextMenu(target)) {
        return;
      }

      const withinCustomMenu = target instanceof HTMLElement && target.closest('[data-app-context-menu-root="true"]');
      if (!withinCustomMenu) {
        return;
      }
    };

    window.addEventListener('contextmenu', handleContextMenu, { capture: true });
    return () => {
      window.removeEventListener('contextmenu', handleContextMenu, { capture: true });
    };
  }, []);

  return null;
};
