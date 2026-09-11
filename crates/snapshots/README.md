# firemage-snapshots

Reads and writes portable `.fmsnap` bundles: a gzip tar archive containing a
versioned manifest, Firecracker state and memory, and private copies of managed
kernel and disk files. The manifest records sizes and SHA-256 hashes.

Extraction accepts only the declared regular files, checks compressed and
expanded size limits, and rejects links, duplicate names and unexpected paths.
The runtime owns catalog authorization, trust decisions and restore compatibility.
