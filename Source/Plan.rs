use std::path::Path;

use anyhow::{Context, Result};
use git2::{
	AutotagOption,
	BranchType,
	FetchOptions,
	MergeOptions,
	PushOptions,
	RemoteCallbacks,
	Repository,
	ResetType,
};
use walkdir::WalkDir;

fn main() -> Result<()> {
	let repo = Repository::open(".").context("Failed to open repository")?;

	restore_gitignore_from_parent(&repo)?;

	restore_package_json_from_parent(&repo)?;

	set_default_repo(&repo)?;

	add_all(&repo)?;

	set_upstream(&repo, "current", "source/current")?;

	set_upstream(&repo, "previous", "source/previous")?;

	clean(&repo)?;

	fetch_from_remote(&repo, "parent", true, 1)?;

	fetch_from_remote(&repo, "source", true, 1)?;

	fetch_unshallow(&repo, "parent")?;

	merge_from_parent(&repo)?;

	pull(&repo)?;

	push(&repo, "source", "HEAD")?;

	push_set_upstream(&repo, "source", "branch", true)?;

	add_remote(&repo, "parent", "$parent")?;

	add_remote(&repo, "source", "$source")?;

	remove_remote(&repo, "parent")?;

	remove_remote(&repo, "origin")?;

	set_remote_url(&repo, "parent", "$parent")?;

	set_remote_url(&repo, "source", "$source")?;

	reset_hard_to_parent(&repo)?;

	reset_file(&repo, "package.json")?;

	restore_file_from_parent(&repo, "package.json")?;

	restore_file_from_parent(&repo, "src")?;

	restore_file_from_parent(&repo, "tsconfig.json")?;

	restore_from_source(&repo, "source/current", "package.json")?;

	restore_file(&repo, "package.json")?;

	add_submodule(&repo, "$origin", "$sub_dependency")?;

	switch_branch(&repo, "$branch")?;

	create_and_switch_branch(&repo, "$branch")?;

	create_and_switch_branch(&repo, "current")?;

	create_and_switch_branch(&repo, "previous")?;

	switch_branch(&repo, "current")?;

	switch_branch(&repo, "previous")?;

	Ok(())
}

// --- Helper Functions ---

fn get_parent_default_branch(repo:&Repository) -> Result<String> {
	let output = Command::new("gh")
		.args(&[
			"repo",
			"view",
			"--json",
			"parent",
			"--jq",
			".defaultBranchRef.name",
		])
		.output()
		.context("Failed to execute 'gh' command")?;

	if !output.status.success() {
		return Err(anyhow::anyhow!(
			"Error getting parent default branch: {}",
			String::from_utf8_lossy(&output.stderr)
		));
	}

	let branch_name = String::from_utf8(output.stdout)
		.context("Invalid UTF-8 in branch name")?
		.trim()
		.to_string();

	Ok(branch_name)
}

fn restore_gitignore_from_parent(repo:&Repository) -> Result<()> {
	restore_files_from_parent(repo, ".gitignore")
}

fn restore_package_json_from_parent(repo:&Repository) -> Result<()> {
	restore_files_from_parent(repo, "package.json")
}

fn restore_files_from_parent(repo:&Repository, filename:&str) -> Result<()> {
	for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
		let path = entry.path();

		if path.file_name().map(|n| n == filename).unwrap_or(false)
			&& !path.starts_with("node_modules")
			&& !path.starts_with(".git")
		{
			restore_file_from_parent(repo, path.to_str().unwrap())?;
		}
	}
	Ok(())
}

fn set_default_repo(repo:&Repository) -> Result<()> {
	let source_url =
		repo.find_remote("source")?.url().unwrap_or_default().to_string();

	Command::new("gh")
		.args(&["repo", "set-default", &source_url])
		.status()
		.context("Failed to set default repo")?;

	Ok(())
}

fn add_all(repo:&Repository) -> Result<()> {
	let mut index = repo.index()?;

	index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;

	index.write()?;

	Ok(())
}

fn set_upstream(
	repo:&Repository,
	local_branch:&str,
	upstream:&str,
) -> Result<()> {
	let mut branch = repo
		.find_branch(local_branch, BranchType::Local)
		.context("Branch not found")?;

	branch.set_upstream(Some(upstream))?;

	Ok(())
}

fn clean(repo:&Repository) -> Result<()> {
	Command::new("git")
		.args(&["clean", "-dfx"])
		.current_dir(repo.path())
		.status()
		.context("Failed to clean repository")?;

	Ok(())
}

fn fetch_from_remote(
	repo:&Repository,
	remote_name:&str,
	no_tags:bool,
	depth:u32,
) -> Result<()> {
	let mut remote =
		repo.find_remote(remote_name).context("Remote not found")?;

	let mut callbacks = RemoteCallbacks::new();

	let mut fetch_options = FetchOptions::new();

	fetch_options.remote_callbacks(callbacks);

	if no_tags {
		fetch_options.download_tags(AutotagOption::None);
	}
	fetch_options.depth(depth);

	remote
		.fetch(&["main"], Some(&mut fetch_options), None)
		.context("Failed to fetch")?;

	Ok(())
}

fn fetch_unshallow(repo:&Repository, remote_name:&str) -> Result<()> {
	Command::new("git")
		.args(&["fetch", remote_name, "--no-tags", "--unshallow"])
		.current_dir(repo.path())
		.status()
		.context("Failed to unshallow fetch")?;

	Ok(())
}

fn merge_from_parent(repo:&Repository) -> Result<()> {
	let parent_branch = get_parent_default_branch(repo)?;

	let parent_commit = repo
		.find_branch(&format!("parent/{}", parent_branch), BranchType::Remote)?
		.get()
		.peel_to_commit()?;

	let head = repo.head()?.peel_to_commit()?;

	repo.merge(
		&[&parent_commit.into_object()],
		Some(MergeOptions::new().allow_unrelated_histories(true)),
		None,
	)
	.context("Failed to merge")?;

	Ok(())
}

fn pull(repo:&Repository) -> Result<()> {
	Command::new("git")
		.args(&[
			"pull",
			"--no-edit",
			"--allow-unrelated-histories",
			"--no-progress",
			"-q",
			"-X",
			"theirs",
		])
		.current_dir(repo.path())
		.status()
		.context("Failed to pull")?;

	Ok(())
}

fn push(repo:&Repository, remote_name:&str, refspec:&str) -> Result<()> {
	let mut remote =
		repo.find_remote(remote_name).context("Remote not found")?;

	let mut callbacks = RemoteCallbacks::new();

	let mut push_options = PushOptions::new();

	push_options.remote_callbacks(callbacks);

	remote
		.push(&[refspec], Some(&mut push_options))
		.context("Failed to push")?;

	Ok(())
}

fn push_set_upstream(
	repo:&Repository,
	remote_name:&str,
	branch:&str,
	force:bool,
) -> Result<()> {
	let refspec = if force {
		format!("+refs/heads/{}:refs/heads/{}", branch, branch)
	} else {
		format!("refs/heads/{}:refs/heads/{}", branch, branch)
	};

	push(repo, remote_name, &refspec)?;

	set_upstream(repo, branch, &format!("{}/{}", remote_name, branch))?;

	Ok(())
}

fn add_remote(repo:&Repository, name:&str, url:&str) -> Result<()> {
	repo.remote(name, url).context("Failed to add remote")?;

	Ok(())
}

fn remove_remote(repo:&Repository, name:&str) -> Result<()> {
	repo.remote_delete(name).context("Failed to remove remote")?;

	Ok(())
}

fn set_remote_url(repo:&Repository, name:&str, url:&str) -> Result<()> {
	repo.remote_set_url(name, url).context("Failed to set remote URL")?;

	Ok(())
}

fn reset_hard_to_parent(repo:&Repository) -> Result<()> {
	let parent_branch = get_parent_default_branch(repo)?;

	let parent_commit = repo
		.find_branch(&format!("parent/{}", parent_branch), BranchType::Remote)?
		.get()
		.peel_to_commit()?;

	repo.reset(&parent_commit.into_object(), ResetType::Hard, None)
		.context("Failed to reset hard")?;

	Ok(())
}

fn reset_file(repo:&Repository, file:&str) -> Result<()> {
	let mut index = repo.index()?;

	index.remove_path(Path::new(file)).context("Failed to reset file")?;

	index.write()?;

	Ok(())
}

fn restore_file_from_parent(repo:&Repository, file_path:&str) -> Result<()> {
	let parent_branch = get_parent_default_branch(repo)?;

	let parent_commit = repo
		.find_branch(&format!("parent/{}", parent_branch), BranchType::Remote)?
		.get()
		.peel_to_commit()?;

	let tree = parent_commit.tree()?;

	let entry = tree.get_path(Path::new(file_path))?;

	let object = entry.to_object(repo)?;

	let blob = object.as_blob().context("Entry is not a blob")?;

	std::fs::write(file_path, blob.content())
		.context("Failed to write file content")?;

	Ok(())
}

fn restore_from_source(repo:&Repository, source:&str, file:&str) -> Result<()> {
	let obj = repo.revparse_single(source)?;

	let tree = obj.peel_to_tree()?;

	let entry = tree.get_path(Path::new(file))?;

	let blob = entry.to_object(repo)?.peel_to_blob()?;

	std::fs::write(file, blob.content())
		.context("Failed to write file content")?;

	Ok(())
}

fn restore_file(repo:&Repository, file:&str) -> Result<()> {
	let head = repo.head()?;

	let tree = head.peel_to_tree()?;

	let entry = tree.get_path(Path::new(file))?;

	let blob = entry.to_object(repo)?.peel_to_blob()?;

	std::fs::write(file, blob.content())
		.context("Failed to write file content")?;

	Ok(())
}

fn add_submodule(
	repo:&Repository,
	origin:&str,
	subdependency:&str,
) -> Result<()> {
	repo.submodule(origin, Path::new(subdependency), false)
		.context("Failed to add submodule")?;

	Ok(())
}

fn switch_branch(repo:&Repository, branch:&str) -> Result<()> {
	repo.set_head(&format!("refs/heads/{}", branch))
		.context("Failed to switch branch")?;

	Ok(())
}

fn create_and_switch_branch(repo:&Repository, branch:&str) -> Result<()> {
	let head = repo.head()?;

	let oid = head.target().unwrap();

	let commit = repo.find_commit(oid)?;

	repo.branch(branch, &commit, false).context("Failed to create branch")?;

	repo.set_head(&format!("refs/heads/{}", branch))
		.context("Failed to switch branch")?;

	Ok(())
}
