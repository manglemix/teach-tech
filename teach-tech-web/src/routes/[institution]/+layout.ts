import { institutions } from '$lib';
import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = ({ params }) => {
    
    if (!(params.institution in institutions)) {
        redirect(307, "/select-institution");
    }
    const host = institutions[params.institution];
	return {
		institution: params.institution,
		host
	};
};