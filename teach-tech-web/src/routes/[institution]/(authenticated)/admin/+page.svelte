<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { handleHttpStatus } from '$lib';
	import type { PageData } from './$types';
	import { DateInput } from 'date-picker-svelte';

	let { data }: { data: PageData } = $props();
	let processing = $state(false);

	let studentsToCreate: { name: string; birthdate: Date; pronouns: string }[] = $state([]);
	let createdStudents: { user_id: string; password: string }[] = $state([]);
	let studentName = $state('');
	let studentBirthday = $state(new Date());
	let studentPronouns = $state('');

	let instructorsToCreate: { name: string; birthdate: Date; pronouns: string }[] = $state([]);
	let createdInstructors: { user_id: string; password: string }[] = $state([]);
	let instructorName = $state('');
	let instructorBirthday = $state(new Date());
	let instructorPronouns = $state('');
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
					Authorization: `Bearer ${data.bearerToken}`
				},
				body: JSON.stringify({ students: studentsToCreate })
			});

			if (resp.ok) {
				studentsToCreate = [];
				createdStudents = (await resp.json()).students;
			} else {
				if (resp.status == 403) {
					alert('Your permissions may have changed');
				}
				if (!handleHttpStatus(resp.status, $page.url, $page.params.institution)) {
					alert('Failed to create students');
				}
			}
			processing = false;
		}}
	>
		<h1>Create Students</h1>
		<label for="student_name" class="mt-4">Student Name</label>
		<input bind:value={studentName} type="text" id="student_name" name="student_name" />
		<label for="student_birthday" class="mt-4">Student Birthday</label>
		<DateInput bind:value={studentBirthday} id="student_birthday" />
		<label for="student_pronouns" class="mt-4">Student Pronouns</label>
		<input bind:value={studentPronouns} type="text" id="student_pronouns" name="student_pronouns" />

		<button
			type="button"
			class="mt-4 rounded bg-blue-500 p-2 text-white"
			onclick={() => {
				if (studentName === '' || studentPronouns === '') {
					alert('Please fill out all fields');
					return;
				}
				studentsToCreate.push({
					name: studentName,
					birthdate: studentBirthday,
					pronouns: studentPronouns
				});
				studentName = '';
				studentBirthday = new Date();
				studentPronouns = '';
			}}>Add Student</button
		>
		{#each studentsToCreate as createStudent}
			<p>
				{createStudent.name} ({createStudent.pronouns}): {createStudent.birthdate.toDateString()}
			</p>
		{/each}

		{#if processing}
			<button type="submit" class="mt-4 rounded bg-blue-500 p-2 text-white" disabled
				>Processing</button
			>
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

<div class="flex flex-col items-center justify-center">
	<form
		class="flex max-w-md flex-col justify-center"
		onsubmit={async (event) => {
			processing = true;
			event.preventDefault();
			const resp = await fetch(`${data.host}/instructor/create`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
					Authorization: `Bearer ${data.bearerToken}`
				},
				body: JSON.stringify({ instructors: instructorsToCreate })
			});

			if (resp.ok) {
				instructorsToCreate = [];
				createdInstructors = (await resp.json()).instructors;
			} else {
				if (resp.status == 403) {
					alert('Your permissions may have changed');
				}
				if (!handleHttpStatus(resp.status, $page.url, $page.params.institution)) {
					alert('Failed to create instructors');
				}
			}
			processing = false;
		}}
	>
		<h1>Create Instructors</h1>
		<label for="instructor_name" class="mt-4">Instructor Name</label>
		<input bind:value={instructorName} type="text" id="instructor_name" name="instructor_name" />
		<label for="instructor_birthday" class="mt-4">Instructor Birthday</label>
		<DateInput bind:value={instructorBirthday} id="instructor_birthday" />
		<label for="instructor_pronouns" class="mt-4">Instructor Pronouns</label>
		<input bind:value={instructorPronouns} type="text" id="instructor_pronouns" name="instructor_pronouns" />

		<button
			type="button"
			class="mt-4 rounded bg-blue-500 p-2 text-white"
			onclick={() => {
				if (instructorName === '' || instructorPronouns === '') {
					alert('Please fill out all fields');
					return;
				}
				instructorsToCreate.push({
					name: instructorName,
					birthdate: instructorBirthday,
					pronouns: instructorPronouns
				});
				instructorName = '';
				instructorBirthday = new Date();
				instructorPronouns = '';
			}}>Add Instructor</button
		>
		{#each instructorsToCreate as createInstructor}
			<p>
				{createInstructor.name} ({createInstructor.pronouns}): {createInstructor.birthdate.toDateString()}
			</p>
		{/each}

		{#if processing}
			<button type="submit" class="mt-4 rounded bg-blue-500 p-2 text-white" disabled
				>Processing</button
			>
		{:else}
			<button type="submit" class="mt-4 rounded bg-blue-500 p-2 text-white">Create</button>
		{/if}
	</form>
</div>

<ul>
	{#each createdInstructors as instructor}
		<li>{instructor.user_id}: {instructor.password}</li>
	{/each}
</ul>
