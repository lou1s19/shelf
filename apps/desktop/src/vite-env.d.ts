/// <reference types="vinxi/types/client" />

interface ImportMetaEnv {
	readonly VITE_SOLID_DEVTOOLS?: string;
	// more env variables...
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
