// One attachment for every `<details class="dropdown">` in the app, so a
// menu closes the way a person expects. daisyUI's details dropdown is
// native open/close and nothing more: a click outside it and Escape are
// both this file's.

import type { Attachment } from 'svelte/attachments';

/// Close a details on a click outside it and on Escape. Setting `open`
/// fires the native toggle, so a `bind:open` on the same element follows.
export function dismiss(): Attachment<HTMLDetailsElement> {
	return (node) => {
		const click = (event: MouseEvent) => {
			if (node.open && event.target instanceof Node && !node.contains(event.target)) {
				node.open = false;
			}
		};
		const keydown = (event: KeyboardEvent) => {
			if (node.open && event.key === 'Escape') node.open = false;
		};
		window.addEventListener('click', click);
		window.addEventListener('keydown', keydown);
		return () => {
			window.removeEventListener('click', click);
			window.removeEventListener('keydown', keydown);
		};
	};
}
