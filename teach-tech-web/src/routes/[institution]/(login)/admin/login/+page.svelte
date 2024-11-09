<script lang=ts>
	import { goto } from "$app/navigation";
	import { page } from "$app/stores";
	import type { PageData } from './$types';

    let url = $state("");
    let user_id = $state("");
    let password = $state("");
	let { data }: { data: PageData } = $props();

    async function onSubmit(event: Event) {
        event.preventDefault();

        // check that user_id is a number
        if (isNaN(parseInt(user_id))) {
            alert("User ID must be a number");
            return;
        }

        if (url.endsWith("/")) {
            url = url.slice(0, -1);
        }

        const resp = await fetch(url + "/auth/login", {
            method: "POST",
            headers: {
                "Content-Type": "application/x-www-form-urlencoded"
            },
            body: encodeURI(`user_id=${user_id}&password=${password}`)
        });

        if (resp.ok) {
            await fetch($page.url.href, {
                method: "POST",
                headers: {
                    "Content-Type": "multipart/form-data"
                },
                body: await resp.text(),
            });
            goto(`/${data.institution}/admin`);
        } else {
            alert("Failed to login");
        }
    }
</script>

<div class="h-screen flex flex-col items-center justify-center w-screen">
    <form class="flex flex-col justify-center max-w-md" onsubmit={onSubmit}>
        <label for="url">URL</label>
        <input bind:value={url} type="url" id="url" name="url" required>
    
        <label for="username" class="mt-4">User ID</label>
        <input bind:value={user_id} type="text" id="user_id" name="user_id" required>
    
        <label for="password" class="mt-4">Password</label>
        <input bind:value={password} type="password" id="password" name="password" required>
        
        <button type="submit" class="mt-4 bg-blue-500 text-white p-2 rounded">Login</button>
    </form>
</div>
