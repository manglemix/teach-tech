import { redirect, type RequestHandler } from '@sveltejs/kit';

export const GET: RequestHandler = async ({ cookies, params }) => {
	cookies.delete('bearer_token', { path: '/' });
	redirect(307, `/${params.institution}/${params.role}/login`);
};