import React, { useState, useEffect } from 'react';
import {
  TabList,
  Tab,
  Text,
  Badge,
  Menu,
  MenuTrigger,
  MenuButton,
  MenuItem,
  MenuPopover,
  MenuList,
  Toolbar,
  ToolbarButton,
} from '@fluentui/react-components';
import {
  Home24Regular,
  Table24Regular,
  Grid24Regular,
  Settings24Regular,
  Info24Regular,
  Person24Regular,
  WeatherMoon24Regular,
  WeatherSunny24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import { HomePage } from '../../pages/Home/HomePage';
import { BillPage } from '../../pages/Bill/BillPage';
import { FeaturesPage } from '../../pages/Features/FeaturesPage';
import { useTheme } from '../../hooks';
import * as tauri from '../../services/tauri';

type TabValue = 'home' | 'bill' | 'features';

export const AppLayout: React.FC = () => {
  const [selectedTab, setSelectedTab] = useState<TabValue>('home');
  const [appVersion, setAppVersion] = useState('');
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const setShowSettingsDialog = useAppStore((s) => s.setShowSettingsDialog);
  const setShowAboutDialog = useAppStore((s) => s.setShowAboutDialog);
  const setShowIdentityManagerDialog = useAppStore((s) => s.setShowIdentityManagerDialog);
  const setShowIdentitySelectDialog = useAppStore((s) => s.setShowIdentitySelectDialog);
  const { theme, toggleTheme } = useTheme();

  // Load app version on mount
  useEffect(() => {
    tauri.get_app_version().then(setAppVersion).catch(() => {});
  }, []);

  const onTabSelect = (_: unknown, data: any) => {
    const val = data?.value as string;
    if (val === 'home' || val === 'bill' || val === 'features') {
      setSelectedTab(val);
    }
  };

  const renderContent = () => {
    switch (selectedTab) {
      case 'home':
        return <HomePage />;
      case 'bill':
        return <BillPage />;
      case 'features':
        return <FeaturesPage />;
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>
      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '8px 20px',
          borderBottom: '1px solid var(--colorNeutralStroke2)',
          flexShrink: 0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <Text weight="semibold" size={400}>
            海大终端
          </Text>
          <TabList selectedValue={selectedTab} onTabSelect={onTabSelect}>
            <Tab icon={<Home24Regular />} value="home">
              首页
            </Tab>
            <Tab icon={<Table24Regular />} value="bill">
              账单
            </Tab>
            <Tab icon={<Grid24Regular />} value="features">
              功能大全
            </Tab>
          </TabList>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          {currentIdentity && (
            <Badge appearance="filled" color="brand" size="medium" style={{ marginRight: 8 }}>
              {currentIdentity.name}
            </Badge>
          )}
          <Toolbar>
            <ToolbarButton
              icon={theme === 'dark' ? <WeatherSunny24Regular /> : <WeatherMoon24Regular />}
              onClick={toggleTheme}
              title={theme === 'dark' ? '切换亮色' : '切换暗色'}
            />
            <Menu>
              <MenuTrigger>
                <MenuButton icon={<Person24Regular />} appearance="subtle">
                  身份
                </MenuButton>
              </MenuTrigger>
              <MenuPopover>
                <MenuList>
                  <MenuItem onClick={() => setShowIdentitySelectDialog(true)}>
                    切换身份
                  </MenuItem>
                  <MenuItem onClick={() => setShowIdentityManagerDialog(true)}>
                    管理身份
                  </MenuItem>
                </MenuList>
              </MenuPopover>
            </Menu>
            <Menu>
              <MenuTrigger>
                <MenuButton appearance="subtle">更多</MenuButton>
              </MenuTrigger>
              <MenuPopover>
                <MenuList>
                  <MenuItem icon={<Settings24Regular />} onClick={() => setShowSettingsDialog(true)}>
                    设置
                  </MenuItem>
                  <MenuItem icon={<Info24Regular />} onClick={() => setShowAboutDialog(true)}>
                    关于
                  </MenuItem>
                </MenuList>
              </MenuPopover>
            </Menu>
          </Toolbar>
        </div>
      </div>

      {/* Content Area */}
      <div style={{ flex: 1, overflow: 'auto', padding: '0' }}>
        {renderContent()}
      </div>

      {/* Status Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '4px 20px',
          borderTop: '1px solid var(--colorNeutralStroke2)',
          fontSize: 12,
          color: 'var(--colorNeutralForeground3)',
          flexShrink: 0,
        }}
      >
        <Text size={200}>
          {currentIdentity ? `当前身份: ${currentIdentity.name}` : '未选择身份'}
        </Text>
        <Text size={200}>
          SHMTU Terminal v{appVersion} | Tauri + React
        </Text>
      </div>
    </div>
  );
};
