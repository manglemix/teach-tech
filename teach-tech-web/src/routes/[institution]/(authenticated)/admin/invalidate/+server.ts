import { invalidateBearerToken } from "$lib";
import type { RequestHandler } from "@sveltejs/kit";

export const GET: RequestHandler = invalidateBearerToken;