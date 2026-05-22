import React, { useEffect } from 'react';
import {
  FluentProvider,
  webLightTheme,
  webDarkTheme,
  Toaster,
} from '@fluentui/react-components';
import { useAppStore } from '../../stores/appStore';
import { AppLayout } from '../Layout/AppLayout';
import { StartupPasswordDialog } from './StartupPasswordDialog';
import { IdentitySelectDialog } from './IdentitySelectDialog';
import { IdentityManagerDialog } from '../../pages/IdentityManager/IdentityManagerDialog';
import { SettingsDialog } from '../../pages/Settings/SettingsDialog';
import { AboutDialog } from '../../pages/About/AboutDialog';
import { CaptchaTestDialog } from '../../pages/CaptchaTest/CaptchaTestDialog';
import { DataTransferDialog } from '../../pages/DataTransfer/DataTransferDialog';
import { StatisticsDialog } from '../../pages/Statistics/StatisticsDialog';

export const AppProvider: React.FC = () => {
  const theme = useAppStore((s) => s.theme);
  const loadIdentities = useAppStore((s) => s.loadIdentities);
  const loadConfig = useAppStore((s) => s.loadConfig);

  useEffect(() => {
    loadIdentities();
    loadConfig();
  }, []);

  const showStartupDialog = useAppStore((s) => s.showStartupDialog);
  const showIdentitySelectDialog = useAppStore((s) => s.showIdentitySelectDialog);
  const showIdentityManagerDialog = useAppStore((s) => s.showIdentityManagerDialog);
  const showSettingsDialog = useAppStore((s) => s.showSettingsDialog);
  const showAboutDialog = useAppStore((s) => s.showAboutDialog);
  const showCaptchaTestDialog = useAppStore((s) => s.showCaptchaTestDialog);
  const showDataTransferDialog = useAppStore((s) => s.showDataTransferDialog);
  const showStatisticsDialog = useAppStore((s) => s.showStatisticsDialog);

  const fluentTheme = theme === 'dark' ? webDarkTheme : webLightTheme;

  return (
    <FluentProvider theme={fluentTheme}>
      <AppLayout />
      <Toaster />

      {/* Modal Dialogs */}
      {showStartupDialog && <StartupPasswordDialog />}
      {showIdentitySelectDialog && <IdentitySelectDialog />}
      {showIdentityManagerDialog && <IdentityManagerDialog />}
      {showSettingsDialog && <SettingsDialog />}
      {showAboutDialog && <AboutDialog />}
      {showCaptchaTestDialog && <CaptchaTestDialog />}
      {showDataTransferDialog && <DataTransferDialog />}
      {showStatisticsDialog && <StatisticsDialog />}
    </FluentProvider>
  );
};