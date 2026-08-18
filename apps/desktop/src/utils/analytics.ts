/**
 * Shelf does not collect usage data.
 *
 * The upstream project sent events to OpenPanel. That is gone. These functions
 * stay so the call sites keep compiling while the features that call them are
 * being reworked; they do nothing and reach no network.
 */

export function initAnonymousUser() {}

export function identifyUser(
	_userId: string,
	_properties?: Record<string, unknown>,
) {}

export function resetUser() {}

export function trackEvent(
	_eventName: string,
	_properties?: Record<string, unknown>,
) {}
