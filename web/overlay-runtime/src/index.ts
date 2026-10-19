/**
 * Light runtime for OBS browser sources (scaffold). Overlays are always served by the local server,
 * never from public pages, because Chromium LNA blocks loopback (ADR-0002, S-2).
 */
export const OVERLAY_RUNTIME_VERSION = "0.0.0";
