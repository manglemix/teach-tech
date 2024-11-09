<script>
	import { institutions } from "$lib";

    let institution = $state("");
</script>

<div id="v-fill" class="flex flex-col items-center">
    <div>
        <select bind:value={institution} class="mt-8">
            <option value="">Select an Institution</option>
            {#each Object.keys(institutions) as name}
                <option value={name}>{name}</option>
            {/each}
        </select>
    
        {#if institution !== ""}
            <h2 class="font-bold mt-6 text-lg">What is your role?</h2>
            <nav class="flex flex-col gap-4 mt-2">
                <a href={`/${institution}/admin/login`}>Administrator</a>
                <a href={`/${institution}/instructor/login`}>Instructor</a>
                <a href={`/${institution}/student/login`}>Student</a>
            </nav>
        {/if}
    </div>
</div>
