import type { LayoutLoad } from './$types';
import { institutions } from '$lib';
import { redirect } from '@sveltejs/kit';

export const load: LayoutLoad = async ({ params, fetch, data }) => {
	const host = institutions[params.institution].url;

	const resp = await fetch(`${host}/instructor/home`, {
		headers: {
			Authorization: `Bearer ${data.bearerToken}`
		}
	});

	if (!resp.ok) {
		if (resp.status === 401) {
			redirect(307, `/${params.institution}/instructor/logout`);
		}
		if (resp.status === 403) {
			redirect(307, `/${params.institution}/instructor/invalid`);
		}
		redirect(307, `/${params.institution}/errors/institution-error`);
	}

	const respData: {
		user_id: string;
		name: string;
		pronouns: string;
		birthdate: Date;
		created_at: Date;
	} = await resp.json();

	return {
		userId: respData.user_id,
		name: respData.name,
		pronouns: respData.pronouns,
		birthdate: respData.birthdate,
		createdAt: respData.created_at,
		bearerToken: data.bearerToken
	};
};
