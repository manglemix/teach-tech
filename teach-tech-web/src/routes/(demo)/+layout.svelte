<script lang="ts">
	let { children } = $props();
</script>

<header>
	<nav class="flex flex-row justify-between">
		<a href="/">
			<h1 class="font-extrabold text-xl">
				Teach Tech
			</h1>
		</a>
		<div class="flex flex-row gap-4 font-bold">
			<a href="/about">About</a>
			<a href="/select-institution">Login</a>
		</div>
	</nav>
</header>

{@render children()}

<footer>
</footer>

<style>
	header {
		background-color: #f3f4f6;
		padding: 1rem;
	}
	footer {
		background-color: #f3f4f6;
		padding: 1rem;
	}
	:global(#v-fill) {
		min-height: calc(100vh - 4rem - 1.75rem);
	}
</style>