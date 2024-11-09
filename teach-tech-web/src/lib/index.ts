import { goto } from '$app/navigation';
import { redirect, type Cookies, type RequestHandler } from '@sveltejs/kit';
import mangle_u_logo from '$lib/assets/mangle_u.png?enhanced';
import type { Picture } from 'vite-imagetools';

// place files you want to import through the `$lib` alias in this folder.
export const institutions: Record<string, { url: string; logo: Picture }> = {
	mangle_u: { url: 'http://127.0.0.1:80', logo: mangle_u_logo }
};

export async function onSubmitForLogin(
	event: Event,
	user_id: string,
	password: string,
	host: string,
	institution: string,
	url: URL
) {
	event.preventDefault();

	// check that user_id is a number
	if (isNaN(parseInt(user_id))) {
		alert('User ID must be a number');
		return;
	}

	const resp = await fetch(host + '/auth/login', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/x-www-form-urlencoded'
		},
		body: encodeURI(`user_id=${user_id}&password=${password}`)
	});

	if (resp.ok) {
		await fetch(url.href, {
			method: 'POST',
			headers: {
				'Content-Type': 'multipart/form-data'
			},
			body: await resp.text()
		});
		const segments = url.pathname.split('/');
		const role = segments[2];
		goto(`/${institution}/${role}`);
	} else {
		alert('Failed to login');
	}
}

export const authenticatedServerLoad = ({
	cookies,
	params,
	url
}: {
	cookies: Cookies;
	params: { institution: string };
	url: URL;
}) => {
	const bearerToken = cookies.get('bearer_token');
	if (!bearerToken) {
		const segments = url.pathname.split('/');
		const role = segments[2];
		redirect(307, `/${params.institution}/${role}/login`);
	}
	return {
		bearerToken
	};
};
