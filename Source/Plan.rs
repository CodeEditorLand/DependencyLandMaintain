use git2::{BranchType, FetchOptions, PushOptions, RemoteCallbacks, Repository, ResetType};
use serde_json::Value;
use std::{path::Path, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let repo = Repository::open(".")?;

	// Restore .gitignore and package.json from parent
	restore_gitignore_from_parent(&repo)?;

	restore_package_json_from_parent(&repo)?;

	// Set default repo using gh CLI
	set_default_repo(&repo)?;

	// Add all changes
	add_all(&repo)?;

	// Set upstream for branches
	set_upstream(&repo, "Current", "Source/Current")?;

	set_upstream(&repo, "Previous", "Source/Previous")?;

	// Clean the repository
	clean(&repo)?;

	// Fetch from remotes
	fetch_from_remote(&repo, "Parent", true, 1)?;

	fetch_from_remote(&repo, "Source", true, 1)?;

	fetch_unshallow("Parent")?;

	// Merge from parent
	merge_from_parent(&repo)?;

	// Pull changes
	pull(&repo)?;

	// Push changes
	push(&repo, "Source", "HEAD")?;

	push_set_upstream(&repo, "Source", "Branch", true)?;

	// Manage remotes
	add_remote(&repo, "Parent", "$Parent")?;

	add_remote(&repo, "Source", "$Source")?;

	remove_remote(&repo, "Parent")?;

	remove_remote(&repo, "origin")?;

	set_remote_url(&repo, "Parent", "$Parent")?;

	set_remote_url(&repo, "Source", "$Source")?;

	// Reset and restore operations
	reset_hard_to_parent(&repo)?;

	reset_file(&repo, "package.json")?;

	restore_file_from_parent(&repo, "package.json")?;

	restore_file_from_parent(&repo, "src")?;

	restore_file_from_parent(&repo, "tsconfig.json")?;

	restore_from_source(&repo, "Source/Current", "package.json")?;

	restore_file(&repo, "package.json")?;

	// Add submodule
	add_submodule("$Origin", "$SubDependency")?;

	// Switch branches
	switch_branch(&repo, "$Branch")?;

	create_and_switch_branch(&repo, "$Branch")?;

	create_and_switch_branch(&repo, "Current")?;

	create_and_switch_branch(&repo, "Previous")?;

	switch_branch(&repo, "Current")?;

	switch_branch(&repo, "Previous")?;

	Ok(())
}

fn restore_gitignore_from_parent(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
	let parent_branch = get_parent_default_branch()?;

	for entry in walkdir::WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
		let path = entry.path();

		if path.file_name().map(|n| n == ".gitignore").unwrap_or(false)
			&& !path.starts_with("node_modules")
			&& !path.starts_with(".git")
		{
			restore_file_from_parent(repo, path.to_str().unwrap())?;
		}
	}
	Ok(())
}

fn restore_package_json_from_parent(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
	let parent_branch = get_parent_default_branch()?;

	for entry in walkdir::WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
		let path = entry.path();

		if path.file_name().map(|n| n == "package.json").unwrap_or(false)
			&& !path.starts_with("node_modules")
			&& !path.starts_with(".git")
		{
			restore_file_from_parent(repo, path.to_str().unwrap())?;
		}
	}
	Ok(())
}

fn set_default_repo(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
	let source_url = repo.find_remote("Source")?.url().unwrap_or_default().to_string();

	Command::new("gh").args(&["repo", "set-default", &source_url]).status()?;

	Ok(())
}

fn add_all(repo: &Repository) -> Result<(), git2::Error> {
	let mut index = repo.index()?;

	index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;

	index.write()?;

	Ok(())
}

fn set_upstream(repo: &Repository, local_branch: &str, upstream: &str) -> Result<(), git2::Error> {
	let mut branch = repo.find_branch(local_branch, BranchType::Local)?;

	branch.set_upstream(Some(upstream))?;

	Ok(())
}

fn clean(repo: &Repository) -> Result<(), std::io::Error> {
	Command::new("git").args(&["clean", "-dfx"]).current_dir(repo.path()).status()?;

	Ok(())
}

fn fetch_from_remote(
	repo: &Repository,
	remote_name: &str,
	no_tags: bool,
	depth: i32,
) -> Result<(), git2::Error> {
	let mut remote = repo.find_remote(remote_name)?;

	let callbacks = RemoteCallbacks::new();

	let mut fetch_options = FetchOptions::new();

	fetch_options.remote_callbacks(callbacks);

	if no_tags {
		fetch_options.download_tags(git2::AutotagOption::None);
	}
	fetch_options.depth(depth);

	remote.fetch(&["main"], Some(&mut fetch_options), None)?;

	Ok(())
}

fn fetch_unshallow(remote: &str) -> Result<(), std::io::Error> {
	Command::new("git").args(&["fetch", remote, "--no-tags", "--unshallow"]).status()?;

	Ok(())
}

fn merge_from_parent(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
	let parent_branch = get_parent_default_branch()?;

	let parent_commit = repo
		.find_branch(&format!("Parent/{}", parent_branch), BranchType::Remote)?
		.get()
		.peel_to_commit()?;

	let head = repo.head()?.peel_to_commit()?;

	repo.merge(
		&[&parent_commit.into_object()],
		None,
		Some(&git2::MergeOptions {
			flags: git2::MergeFlags::ALLOW_UNRELATED_HISTORIES,
			..Default::default()
		}),
	)?;

	Ok(())
}

fn pull(repo: &Repository) -> Result<(), std::io::Error> {
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
		.status()?;

	Ok(())
}

fn push(repo: &Repository, remote_name: &str, refspec: &str) -> Result<(), git2::Error> {
	let mut remote = repo.find_remote(remote_name)?;

	let mut callbacks = RemoteCallbacks::new();

	let mut push_options = PushOptions::new();

	push_options.remote_callbacks(callbacks);

	remote.push(&[refspec], Some(&mut push_options))?;

	Ok(())
}

fn push_set_upstream(
	repo: &Repository,
	remote_name: &str,
	branch: &str,
	force: bool,
) -> Result<(), git2::Error> {
	let refspec = if force {
		format!("+refs/heads/{}:refs/heads/{}", branch, branch)
	} else {
		format!("refs/heads/{}:refs/heads/{}", branch, branch)
	};

	push(repo, remote_name, &refspec)?;

	set_upstream(repo, branch, &format!("{}/{}", remote_name, branch))?;

	Ok(())
}

fn add_remote(repo: &Repository, name: &str, url: &str) -> Result<(), git2::Error> {
	repo.remote(name, url)?;

	Ok(())
}

fn remove_remote(repo: &Repository, name: &str) -> Result<(), git2::Error> {
	repo.remote_delete(name)?;

	Ok(())
}

fn set_remote_url(repo: &Repository, name: &str, url: &str) -> Result<(), git2::Error> {
	repo.remote_set_url(name, url)?;

	Ok(())
}

fn reset_hard_to_parent(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
	let parent_branch = get_parent_default_branch()?;

	let parent_commit = repo
		.find_branch(&format!("Parent/{}", parent_branch), BranchType::Remote)?
		.get()
		.peel_to_commit()?;

	repo.reset(&parent_commit.into_object(), ResetType::Hard, None)?;

	Ok(())
}

fn reset_file(repo: &Repository, file: &str) -> Result<(), git2::Error> {
	let mut index = repo.index()?;

	index.remove_path(Path::new(file))?;

	index.write()?;

	Ok(())
}

fn restore_file_from_parent(
	repo: &Repository,
	file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	let parent_branch = get_parent_default_branch()?;

	let parent_commit = repo
		.find_branch(&format!("Parent/{}", parent_branch), BranchType::Remote)?
		.get()
		.peel_to_commit()?;

	let tree = parent_commit.tree()?;

	let entry = tree.get_path(Path::new(file_path))?;

	let object = entry.to_object(repo)?;

	let blob = object.as_blob().ok_or("Not a blob")?;

	std::fs::write(file_path, blob.content())?;

	Ok(())
}

fn restore_from_source(repo: &Repository, source: &str, file: &str) -> Result<(), git2::Error> {
	let obj = repo.revparse_single(source)?;

	let tree = obj.peel_to_tree()?;

	let entry = tree.get_path(Path::new(file))?;

	let blob = entry.to_object(repo)?.peel_to_blob()?;

	std::fs::write(file, blob.content())?;

	Ok(())
}

fn restore_file(repo: &Repository, file: &str) -> Result<(), git2::Error> {
	let head = repo.head()?;

	let tree = head.peel_to_tree()?;

	let entry = tree.get_path(Path::new(file))?;

	let blob = entry.to_object(repo)?.peel_to_blob()?;

	std::fs::write(file, blob.content())?;

	Ok(())
}

fn add_submodule(origin: &str, subdependency: &str) -> Result<(), std::io::Error> {
	Command::new("git").args(&["submodule", "add", "--depth=1", origin, subdependency]).status()?;

	Ok(())
}

fn switch_branch(repo: &Repository, branch: &str) -> Result<(), git2::Error> {
	repo.set_head(&format!("refs/heads/{}", branch))?;

	Ok(())
}

fn create_and_switch_branch(repo: &Repository, branch: &str) -> Result<(), git2::Error> {
	let head = repo.head()?;

	let oid = head.target().unwrap();

	let commit = repo.find_commit(oid)?;

	repo.branch(branch, &commit, false)?;

	repo.set_head(&format!("refs/heads/{}", branch))?;

	Ok(())
}

fn get_parent_default_branch() -> Result<String, Box<dyn std::error::Error>> {
	let parent_info = Command::new("gh").args(&["repo", "view", "--json", "parent"]).output()?;

	let parent_info: Value = serde_json::from_slice(&parent_info.stdout)?;

	let parent_repo = format!(
		"{}/{}",
		parent_info["parent"]["owner"]["login"].as_str().unwrap(),
		parent_info["parent"]["name"].as_str().unwrap()
	);

	let branch_info = Command::new("gh")
		.args(&["repo", "view", &parent_repo, "--json", "defaultBranchRef"])
		.output()?;

	let branch_info: Value = serde_json::from_slice(&branch_info.stdout)?;

	Ok(branch_info["defaultBranchRef"]["name"].as_str().unwrap().to_string())
}
