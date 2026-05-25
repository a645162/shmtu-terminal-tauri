import React from 'react';
import {
  Card,
  CardHeader,
  Text,
  Title3,
  Subtitle2,
} from '@fluentui/react-components';
import {
  ChartMultiple24Regular,
  ArrowExport24Regular,
  ArrowImport24Regular,
  ShieldTask24Regular,
  Person24Regular,
  Settings24Regular,
  Camera24Regular,
  BookInformation24Regular,
  ArrowSync24Regular,
  Water24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';
import {
  CardEnterMotion,
  SectionEnterMotion,
  getStaggerDelay,
} from '../../components/Common/motion';

interface FeatureItem {
  title: string;
  description: string;
  icon: React.ReactNode;
  disabled?: boolean;
  action: () => void;
}

export const FeaturesPage: React.FC = () => {
  const setShowStatisticsDialog = useAppStore((s) => s.setShowStatisticsDialog);
  const setShowDataTransferDialog = useAppStore((s) => s.setShowDataTransferDialog);
  const setShowCaptchaTestDialog = useAppStore((s) => s.setShowCaptchaTestDialog);
  const setShowIdentityManagerDialog = useAppStore((s) => s.setShowIdentityManagerDialog);
  const setShowSettingsDialog = useAppStore((s) => s.setShowSettingsDialog);
  const setShowAboutDialog = useAppStore((s) => s.setShowAboutDialog);

  const features: FeatureItem[] = [
    {
      title: '统计分析',
      description: '消费趋势、分类占比、时段分布',
      icon: <ChartMultiple24Regular />,
      action: () => setShowStatisticsDialog(true),
    },
    {
      title: '数据导出',
      description: 'CSV / JSON / 钱迹格式',
      icon: <ArrowExport24Regular />,
      action: () => setShowDataTransferDialog(true),
    },
    {
      title: '数据导入',
      description: '从JSON文件导入数据',
      icon: <ArrowImport24Regular />,
      action: () => setShowDataTransferDialog(true),
    },
    {
      title: '验证码测试',
      description: '测试OCR服务器和本地ONNX',
      icon: <ShieldTask24Regular />,
      action: () => setShowCaptchaTestDialog(true),
    },
    {
      title: '身份管理',
      description: '管理身份和账号',
      icon: <Person24Regular />,
      action: () => setShowIdentityManagerDialog(true),
    },
    {
      title: '应用设置',
      description: '启动保护、验证码、同步设置',
      icon: <Settings24Regular />,
      action: () => setShowSettingsDialog(true),
    },
    {
      title: '数据快照',
      description: '创建和恢复数据快照',
      icon: <Camera24Regular />,
      action: () => setShowDataTransferDialog(true),
    },
    {
      title: '关于',
      description: '版本信息和开发者',
      icon: <BookInformation24Regular />,
      action: () => setShowAboutDialog(true),
    },
    {
      title: '检查更新',
      description: '检查是否有新版本',
      icon: <ArrowSync24Regular />,
      action: async () => {
        try {
          const result = await tauri.check_for_updates();
          if (result) {
            alert(`发现新版本: ${result}`);
          } else {
            alert('当前已是最新版本');
          }
        } catch {
          alert('检查更新失败，请稍后重试');
        }
      },
    },
    {
      title: '热水查询',
      description: 'API维护中，暂不可用',
      icon: <Water24Regular />,
      disabled: true,
      action: () => {},
    },
  ];

  return (
    <div style={{ padding: 20, maxWidth: 1200, margin: '0 auto' }}>
      <SectionEnterMotion>
        <div>
          <Title3 block style={{ marginBottom: 16 }}>
            功能大全
          </Title3>
        </div>
      </SectionEnterMotion>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
          gap: 12,
        }}
      >
        {features.map((feature, index) => (
          <CardEnterMotion key={feature.title} delay={getStaggerDelay(index, 65, 80)}>
            <Card
              className={feature.disabled ? 'motion-sheen' : 'motion-hover-lift motion-sheen'}
              style={{
                cursor: feature.disabled ? 'not-allowed' : 'pointer',
                opacity: feature.disabled ? 0.5 : 1,
                padding: 20,
              }}
              onClick={feature.disabled ? undefined : feature.action}
            >
              <div className="motion-float" style={{ fontSize: 32, marginBottom: 8, color: 'var(--colorBrandForeground1)' }}>
                {feature.icon}
              </div>
              <Subtitle2 block>{feature.title}</Subtitle2>
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                {feature.description}
              </Text>
              {feature.disabled && (
                <Text
                  size={100}
                  style={{
                    color: 'var(--colorPaletteRedForeground3)',
                    marginTop: 4,
                    display: 'block',
                  }}
                >
                  API维护中
                </Text>
              )}
            </Card>
          </CardEnterMotion>
        ))}
      </div>
    </div>
  );
};
