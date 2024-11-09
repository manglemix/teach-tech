<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import type { PageData } from './$types';
	import { DateInput } from 'date-picker-svelte'

	let { data }: { data: PageData } = $props();
	let studentsToCreate: { name: string, birthday: Date, pronouns: string }[] = $state([]);
	let createdStudents: { user_id: string, password: string }[] = $state([]);
	let processing = $state(false);
	let studentName = $state('');
	let studentBirthday = $state(new Date());
	let studentPronouns = $state('');
</script>

<h1>Welcome {data.username}</h1>
<h2>Notifications</h2>
{#each data.adminNotifications as notification}
	<p>{notification.severity}: {notification.msg}</p>
{/each}

<div class="flex flex-col items-center justify-center">
	<form
		class="flex max-w-md flex-col justify-center"
		onsubmit={async (event) => {
			processing = true;
			event.preventDefault();
			const resp = await fetch(`${data.host}/student/create`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
					Authorization: `Bearer ${data.bearerToken}`,
				},
				body: JSON.stringify({ students: studentsToCreate }),
			});

			if (resp.ok) {
				studentsToCreate = [];
				createdStudents = (await resp.json()).students;
			} else if (resp.status === 401) {
				const segments = $page.url.pathname.split("/");
				const role = segments[2];
				goto(`${$page.params.institution}/${role}/invalidate`);
			} else if (resp.status === 403) {
				alert('Your permissions may have changed');
				const segments = $page.url.pathname.split("/");
				const role = segments[2];
				goto(`${$page.params.institution}/${role}/invalidate`);
			} else {
				alert('Failed to create students');
			}
			processing = false;
		}}
	>
		<h1>Create Students</h1>
		<label for="student_count" class="mt-4">Student Name</label>
		<input bind:value={studentName} type="text" id="student_name" name="student_name" />
		<label for="student_birthday" class="mt-4">Student Birthday</label>
		<DateInput bind:value={studentBirthday} id="student_birthday" />
		<label for="student_count" class="mt-4">Student Pronouns</label>
		<input bind:value={studentPronouns} type="text" id="student_pronouns" name="student_pronouns" />

		<button type="button" class="mt-4 rounded bg-blue-500 p-2 text-white" onclick={() => {
			if (studentName === '' || studentPronouns === '') {
				alert('Please fill out all fields');
				return;
			}
			studentsToCreate.push({ name: studentName, birthday: studentBirthday, pronouns: studentPronouns });
			studentName = '';
			studentBirthday = new Date();
			studentPronouns = '';
		}}>Add Student</button>
		{#each studentsToCreate as createStudent}
			<p>{createStudent.name} ({createStudent.pronouns}): {createStudent.birthday.toDateString()}</p>
		{/each}

		{#if processing}
			<button type="submit" class="mt-4 rounded bg-blue-500 p-2 text-white" disabled>Processing</button>
		{:else}
			<button type="submit" class="mt-4 rounded bg-blue-500 p-2 text-white">Create</button>
		{/if}
	</form>
</div>

<ul>
{#each createdStudents as student}
	<li>{student.user_id}: {student.password}</li>
{/each}
</ul>