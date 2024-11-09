import type { LayoutServerLoad } from './$types';
import { authenticatedServerLoad } from '$lib';

export const load: LayoutServerLoad = authenticatedServerLoad;