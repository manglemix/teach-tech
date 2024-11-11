import type { LayoutLoad } from './$types';
import { institutions } from '$lib';
import { redirect } from '@sveltejs/kit';

export const load: LayoutLoad = async ({ params, fetch, data }) => {
	const host = institutions[params.institution].url;

	const resp = await fetch(`${host}/admin/home`, {
		headers: {
			Authorization: `Bearer ${data.bearerToken}`
		}
	});

	if (!resp.ok) {
		if (resp.status === 401) {
			redirect(307, `/${params.institution}/admin/logout`);
		}
		if (resp.status === 403) {
			redirect(307, `/${params.institution}/admin/invalid`);
		}
		redirect(307, `/${params.institution}/errors/institution-error`);
	}

	const respData: {
		user_id: string;
		username: string;
		admin_notifications: { msg: string; severity: string }[];
	} = await resp.json();

	return {
		userId: respData.user_id,
		username: respData.username,
		adminNotifications: respData.admin_notifications,
		bearerToken: data.bearerToken
	};
};
