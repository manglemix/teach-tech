import type { LayoutLoad } from './$types';
import { institutions } from '$lib';

export const load: LayoutLoad = async ({ params, data }) => {
	const host = institutions[params.institution].url;

	const resp = await fetch(`${host}/admin/home`, {
		headers: {
			Authorization: `Bearer ${data.bearerToken}`,
		},
	});

	const respData: { user_id: string, admin_notifications: { msg: string, severity: string }[] } = await resp.json();

	return {
		userId: respData.user_id,
		adminNotifications: respData.admin_notifications,
		bearerToken: data.bearerToken,
	};
};
