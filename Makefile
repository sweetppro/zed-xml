build:
	mkdir -p target/xml
	zed-extension --source-dir . --output-dir target/ --scratch-dir target/
	tar -xzf target/archive.tar.gz -C target/xml
	cp -Rf target/xml ~/Library/Application\ Support/Zed/extensions/installed/
	tree ~/Library/Application\ Support/Zed/extensions/installed/xml
