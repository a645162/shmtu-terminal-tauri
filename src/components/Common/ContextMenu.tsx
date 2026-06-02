import React from 'react';
import * as ContextMenuPrimitive from '@radix-ui/react-context-menu';
import { Text } from '@fluentui/react-components';

export interface ContextMenuAction {
  key: string;
  label: string;
  onSelect: () => void | Promise<void>;
  danger?: boolean;
  disabled?: boolean;
}

interface ContextMenuProps {
  actions: ContextMenuAction[];
  children: React.ReactNode;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ actions, children }) => {
  if (actions.length === 0) {
    return <>{children}</>;
  }

  const portalContainer =
    typeof document !== 'undefined'
      ? document.getElementById('app-context-menu-portal')
      : null;

  return (
    <ContextMenuPrimitive.Root>
      <ContextMenuPrimitive.Trigger asChild>
        {children as React.ReactElement}
      </ContextMenuPrimitive.Trigger>
      <ContextMenuPrimitive.Portal container={portalContainer}>
        <ContextMenuPrimitive.Content
          className="app-context-menu"
        >
          {actions.map((action) => (
            <ContextMenuPrimitive.Item
              key={action.key}
              disabled={action.disabled}
              className={`app-context-menu-item${action.danger ? ' danger' : ''}`}
              onSelect={action.onSelect}
            >
              <Text size={200}>{action.label}</Text>
            </ContextMenuPrimitive.Item>
          ))}
        </ContextMenuPrimitive.Content>
      </ContextMenuPrimitive.Portal>
    </ContextMenuPrimitive.Root>
  );
};
