export * from './upstream/bruno-common/src/utils/type-helpers';
export * from './upstream/bruno-common/src/utils/datatype';
export * from './upstream/bruno-common/src/utils/environment-inheritance';
// Identical normalization contract to upstream bruno-common/src/utils/index.ts.
export const TIMEOUT_INHERIT = 'inherit';
export const resolveTimeoutSetting = (value: unknown) => value === 'inherit' ? value : typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0;
export const parseMaxRedirects = (value: unknown) => typeof value === 'number' && Number.isFinite(value) && value >= 0 ? Math.trunc(value) : undefined;
export const toMaxRedirects = (value: unknown) => parseMaxRedirects(value) ?? 5;
