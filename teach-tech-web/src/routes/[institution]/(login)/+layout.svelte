<script lang=ts>
	import { page } from "$app/stores";
	import { institutions } from "$lib";

	let { children } = $props();
</script>

<header>
    <nav>
        <a href={`/select-institution?inst=${$page.params.institution}`} aria-label={$page.params.institution}>
            <enhanced:img src={institutions[$page.params.institution].logo} alt={$page.params.institution} class="h-16 w-auto" />
        </a>
    </nav>
</header>
{@render children()}

<style>
    header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.5rem;
        background-color: #f3f4f6;
    }
</style>