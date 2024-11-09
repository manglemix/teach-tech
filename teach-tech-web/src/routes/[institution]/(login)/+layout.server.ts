import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = ({ cookies, params, url }) => {
	const bearerToken = cookies.get('bearer_token');
	if (bearerToken) {
		const segments = url.pathname.split('/');
		const role = segments[2];
		redirect(307, `/${params.institution}/${role}/`);
	}
	return {};
};
