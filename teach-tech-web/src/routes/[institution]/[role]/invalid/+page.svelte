<script lang=ts>
	import { page } from "$app/stores";
</script>

<h1>You cannot log in as {$page.params.role}</h1>
<a href={`/${$page.params.institution}/${$page.params.role}/logout`}>Logout?</a>