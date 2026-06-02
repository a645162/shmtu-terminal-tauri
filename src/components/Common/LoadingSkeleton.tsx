import React from 'react';

export const SkeletonLine: React.FC<{
  width?: number | string;
  height?: number;
  radius?: number;
}> = ({ width = '100%', height = 12, radius = 8 }) => (
  <div
    className="motion-skeleton"
    style={{
      width,
      height,
      borderRadius: radius,
    }}
  />
);

export const SkeletonStatGrid: React.FC<{ count?: number }> = ({ count = 6 }) => (
  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 16, marginTop: 8 }}>
    {Array.from({ length: count }).map((_, index) => (
      <div key={index} style={{ display: 'grid', gap: 8 }}>
        <SkeletonLine width="48%" height={11} />
        <SkeletonLine width="72%" height={22} radius={10} />
      </div>
    ))}
  </div>
);

export const SkeletonCardGrid: React.FC<{ count?: number; minWidth?: number }> = ({
  count = 6,
  minWidth = 150,
}) => (
  <div
    style={{
      display: 'grid',
      gridTemplateColumns: `repeat(auto-fill, minmax(${minWidth}px, 1fr))`,
      gap: 8,
      marginTop: 8,
    }}
  >
    {Array.from({ length: count }).map((_, index) => (
      <div
        key={index}
        style={{
          padding: '10px 12px',
          borderRadius: 8,
          border: '1px solid var(--colorNeutralStroke2)',
          background: 'var(--colorNeutralBackground1)',
          display: 'grid',
          gap: 8,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <SkeletonLine width={8} height={8} radius={999} />
          <SkeletonLine width="44%" height={11} />
        </div>
        <SkeletonLine width="70%" height={20} radius={10} />
        <SkeletonLine width="90%" height={10} />
      </div>
    ))}
  </div>
);

export const SkeletonChartBlock: React.FC<{ height?: number }> = ({ height = 220 }) => (
  <div
    style={{
      height,
      display: 'grid',
      alignItems: 'end',
      gap: 10,
      paddingTop: 8,
    }}
  >
    <SkeletonLine width="88%" height={height - 36} radius={16} />
    <SkeletonLine width="42%" height={10} />
  </div>
);

export const SkeletonTableRows: React.FC<{ rows?: number }> = ({ rows = 4 }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    {Array.from({ length: rows }).map((_, index) => (
      <div
        key={index}
        style={{
          display: 'grid',
          gridTemplateColumns: '1.2fr 1fr 1fr 0.7fr 0.7fr 0.5fr 40px',
          gap: 12,
          alignItems: 'center',
          padding: '10px 0',
          borderBottom: '1px solid var(--colorNeutralStroke2)',
        }}
      >
        <SkeletonLine width="86%" height={12} />
        <SkeletonLine width="78%" height={12} />
        <SkeletonLine width="80%" height={12} />
        <SkeletonLine width="70%" height={12} />
        <SkeletonLine width="72%" height={12} />
        <SkeletonLine width="64%" height={20} radius={999} />
        <SkeletonLine width={24} height={24} radius={999} />
      </div>
    ))}
  </div>
);
