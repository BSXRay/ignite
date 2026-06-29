plugins {
    java
    id("io.papermc.paperweight.userdev") version "1.7.1"
}

group = "com.ignite"
version = "1.0.0"
description = "Ignite Backup & Sync Plugin"

java {
    toolchain.languageVersion.set(JavaLanguageVersion.of(21))
}

repositories {
    mavenCentral()
    maven("https://repo.papermc.io/repository/maven-public/")
}

dependencies {
    paperweight.paperApi("1.21.3-R0.1-SNAPSHOT")
    implementation("org.json:json:20231013")
}

tasks {
    assemble {
        dependsOn(reobfJar)
    }

    compileJava {
        options.encoding = "UTF-8"
        options.release.set(21)
    }
}
