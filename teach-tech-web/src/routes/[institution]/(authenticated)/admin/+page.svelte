<script lang=ts>
	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();
</script>

<h1>Welcome {data.username}</h1>
<h2>Notifications</h2>
{#each data.adminNotifications as notification}
    <p>{notification.severity}:{notification.msg}</p>
{/each}