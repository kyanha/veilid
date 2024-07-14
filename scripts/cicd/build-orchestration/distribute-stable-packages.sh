#!/bin/bash

# Clean and reset the workspaces
echo "Setting up the workspace"
# Rsync active repo to local workspace
rsync --archive gitlab-runner@10.116.0.3:/srv $HOME
# Ensure repo directory structure exists
mkdir -p $HOME/srv/{gpg,rpm/{nightly/x86_64,nightly/x86_64,stable/x86_64,stable/x86_64},apt/{dists/{stable/main/{binary-amd64,binary-arm64},nightly/main/{binary-amd64,binary-arm64}},pool/{stable/main,nightly/main}}}
# Delete previous versions of packages
rm -rf $HOME/srv/apt/pool/stable/main/*.deb
rm -rf $HOME/srv/rpm/{stable/x86_64/*,stable/x86_64/*}
# Ensure RPM workspace setup
mkdir -p $HOME/rpm-build-container/mount/repo/{nightly/x86_64,nightly/x86_64,stable/x86_64,stable/x86_64}
rm -rf $HOME/rpm-builder/mount/repo/{stable/x86_64/*,stable/x86_64/*}

# Setup crypto
export GNUPGHOME="$(mktemp -d ~/pgpkeys-XXXXXX)"
cat $HOME/package-signing-key.private | gpg --import
gpg --armor --export admin@veilid.org > $HOME/srv/gpg/veilid-packages-key.public

# Copy .deb files into the workspace and generate repo files
echo "Starting deb process"
cd $HOME
tar -xf amd64-debs.tar
tar -xf arm64-debs.tar
cp *.deb $HOME/srv/apt/pool/stable/main
cd $HOME/srv/apt
echo "Creating Packages file"
dpkg-scanpackages --arch amd64 pool/stable > dists/stable/main/binary-amd64/Packages
dpkg-scanpackages --arch arm64 pool/stable > dists/stable/main/binary-arm64/Packages
cat dists/stable/main/binary-amd64/Packages | gzip -9 > dists/stable/main/binary-amd64/Packages.gz
cat dists/stable/main/binary-arm64/Packages | gzip -9 > dists/stable/main/binary-arm64/Packages.gz
echo "Creating Release file"
cd $HOME/srv/apt/dists/stable
~/generate-release.sh > Release
echo "Signing Release file and creating InRelease"
cat $HOME/srv/apt/dists/stable/Release | gpg --default-key admin@veilid.org -abs > $HOME/srv/apt/dists/stable/Release.gpg
cat $HOME/srv/apt/dists/stable/Release | gpg --default-key admin@veilid.org -abs --clearsign > $HOME/srv/apt/dists/stable/InRelease

# Copy .rpm files into the workspace and generate repo files
echo "Starting rpm process"
cd $HOME
tar -xf amd64-rpms.tar
echo "Copying rpms to container workspace"
cp *x86_64.rpm $HOME/rpm-build-container/mount/repo/stable/x86_64
echo "Copying signing material to container workspace"
cp -R $GNUPGHOME $HOME/rpm-build-container/mount/keystore
echo "Executing container actions"
docker run --rm -d -it --name rpm-repo-builder --mount type=bind,source=$HOME/rpm-build-container/mount,target=/mount rpm-repo-builder-img:v8
sleep 2
cp -R $HOME/rpm-build-container/mount/repo/stable ~/srv/rpm/stable
cd $HOME/srv/rpm/stable/x86_64
echo "Signing the rpm repository"
gpg --default-key admin@veilid.org --detach-sign --armor $HOME/srv/rpm/stable/x86_64/repodata/repomd.xml

echo "[veilid-stable-x86_64-rpm-repo]
name=Veilid Stable x86_64 RPM Repo
baseurl=https://packages.veilid.net/rpm/stable/x86_64
enabled=1
gpgcheck=1
gpgkey=https://packages.veilid.net/gpg/veilid-packages-key.public" > $HOME/srv/rpm/stable/x86_64/veilid-rpm-repo.repo

# Tar the repo data and transfer to the repo server
echo "Moving the repo scaffold to the repo server"
cd $HOME
rsync --archive $HOME/srv gitlab-runner@10.116.0.3:/srv
# tar -cf $HOME/repo.tar srv
# scp -i $HOME/.ssh/id_ed25519 $HOME/repo.tar gitlab-runner@10.116.0.3:~

# Cleanup
echo "Cleaning up the workspace"
rm -rf $GNUPGHOME
# rm $HOME/repo.tar
rm $HOME/*.deb
rm $HOME/*.rpm
rm -rf $HOME/rpm-build-container/mount/keystore
# rm rpm-build-container/mount/repo/*.rpm
# rm -rf rpm-build-container/mount/repo/repodata/*
echo "Process complete"