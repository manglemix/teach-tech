<script lang="ts">
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<h1>Welcome {data.name} ({data.pronouns})</h1>
<h1>Born {data.birthdate}</h1>
<h1>Joined {data.createdAt}</h1>
