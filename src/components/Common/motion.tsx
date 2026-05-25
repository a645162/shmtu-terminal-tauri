import {
  createMotionComponent,
  createMotionComponentVariant,
  curves,
  durations,
} from '@fluentui/react-motion';

type EnterMotionParams = {
  delay?: number;
  duration?: number;
  offsetX?: number;
  offsetY?: number;
  scaleFrom?: number;
};

const enterMotion = createMotionComponent<EnterMotionParams>(
  ({
    delay = 0,
    duration = durations.durationSlower,
    offsetX = 0,
    offsetY = 16,
    scaleFrom = 0.98,
  }) => ({
    keyframes: [
      {
        opacity: 0,
        transform: `translate3d(${offsetX}px, ${offsetY}px, 0) scale(${scaleFrom})`,
      },
      {
        opacity: 1,
        transform: 'translate3d(0, 0, 0) scale(1)',
      },
    ],
    duration,
    delay,
    easing: curves.curveDecelerateMax,
    fill: 'both',
    reducedMotion: {
      keyframes: [{ opacity: 0 }, { opacity: 1 }],
      duration: durations.durationFast,
      delay,
      easing: curves.curveEasyEase,
      fill: 'both',
    },
  })
);

export const PageEnterMotion = createMotionComponentVariant(enterMotion, {
  duration: durations.durationUltraSlow,
  offsetY: 24,
  scaleFrom: 0.985,
});

export const SectionEnterMotion = createMotionComponentVariant(enterMotion, {
  duration: durations.durationSlow,
  offsetY: 18,
});

export const CardEnterMotion = createMotionComponentVariant(enterMotion, {
  duration: durations.durationGentle,
  offsetY: 14,
  scaleFrom: 0.97,
});

export const SlideInFromRightMotion = createMotionComponentVariant(enterMotion, {
  duration: durations.durationSlow,
  offsetX: 20,
  offsetY: 0,
  scaleFrom: 1,
});

export const PopInMotion = createMotionComponentVariant(enterMotion, {
  duration: durations.durationGentle,
  offsetY: 0,
  scaleFrom: 0.92,
});

export function getStaggerDelay(index: number, step = 70, base = 0): number {
  return base + index * step;
}
