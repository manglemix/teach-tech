<script lang="ts">
	import { page } from '$app/stores';
	import { onSubmitForLogin } from '$lib';
	import type { PageData } from '../$types';

	let user_id = $state('');
	let password = $state('');
	let { data }: { data: PageData } = $props();
</script>

<div class="flex h-screen w-screen flex-col items-center justify-center">
	<form
		class="flex max-w-md flex-col justify-center"
		onsubmit={(event) => onSubmitForLogin(event, user_id, password, data, $page.url.href)}
	>
		<label for="username" class="mt-4">User ID</label>
		<input bind:value={user_id} type="text" id="user_id" name="user_id" required />

		<label for="password" class="mt-4">Password</label>
		<input bind:value={password} type="password" id="password" name="password" required />

		<button type="submit" class="mt-4 rounded bg-blue-500 p-2 text-white">Login</button>
	</form>
</div>
