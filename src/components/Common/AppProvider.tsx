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
import { ManualCaptchaDialog } from './ManualCaptchaDialog';

export const AppProvider: React.FC = () => {
  const theme = useAppStore((s) => s.theme);
  const loadIdentities = useAppStore((s) => s.loadIdentities);
  const loadConfig = useAppStore((s) => s.loadConfig);
  const setShowStartupDialog = useAppStore((s) => s.setShowStartupDialog);
  const config = useAppStore((s) => s.config);

  useEffect(() => {
    const init = async () => {
      await loadConfig();
    };
    init();
  }, []);

  // Enable startup protection if configured
  useEffect(() => {
    if (config?.security?.enable_startup_protection) {
      setShowStartupDialog(true);
    }
  }, [config, setShowStartupDialog]);

  const showStartupDialog = useAppStore((s) => s.showStartupDialog);
  const showIdentitySelectDialog = useAppStore((s) => s.showIdentitySelectDialog);
  const showIdentityManagerDialog = useAppStore((s) => s.showIdentityManagerDialog);
  const showSettingsDialog = useAppStore((s) => s.showSettingsDialog);
  const showAboutDialog = useAppStore((s) => s.showAboutDialog);
  const showCaptchaTestDialog = useAppStore((s) => s.showCaptchaTestDialog);
  const showDataTransferDialog = useAppStore((s) => s.showDataTransferDialog);
  const showStatisticsDialog = useAppStore((s) => s.showStatisticsDialog);
  const showManualCaptchaDialog = useAppStore((s) => s.showManualCaptchaDialog);
  const captchaImage = useAppStore((s) => s.captchaImage);
  const captchaExecution = useAppStore((s) => s.captchaExecution);
  const setShowManualCaptchaDialog = useAppStore((s) => s.setShowManualCaptchaDialog);

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
      {showManualCaptchaDialog && captchaImage && captchaExecution && (
        <ManualCaptchaDialog
          captchaImage={captchaImage}
          execution={captchaExecution}
          onSuccess={() => setShowManualCaptchaDialog(false)}
          onCancel={() => setShowManualCaptchaDialog(false)}
        />
      )}
    </FluentProvider>
  );
};
