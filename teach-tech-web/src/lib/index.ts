import { goto } from '$app/navigation';

// place files you want to import through the `$lib` alias in this folder.
export const institutions: Record<string, { url: string }> = {
	mangle_u: { url: 'http://127.0.0.1:80' }
};

export async function onSubmitForLogin(
	event: Event,
	user_id: string,
	password: string,
	data: { host: string; institution: string },
	href: string
) {
	event.preventDefault();

	// check that user_id is a number
	if (isNaN(parseInt(user_id))) {
		alert('User ID must be a number');
		return;
	}

	const resp = await fetch(data.host + '/auth/login', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/x-www-form-urlencoded'
		},
		body: encodeURI(`user_id=${user_id}&password=${password}`)
	});

	if (resp.ok) {
		await fetch(href, {
			method: 'POST',
			headers: {
				'Content-Type': 'multipart/form-data'
			},
			body: await resp.text()
		});
		goto(`/${data.institution}/admin`);
	} else {
		alert('Failed to login');
	}
}
