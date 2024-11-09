import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	const data: { token: string; expires: Date } = await request.json();
	cookies.set('bearer_token', data.token, { expires: data.expires, path: '/' });
	return new Response(null, { status: 204 });
};
