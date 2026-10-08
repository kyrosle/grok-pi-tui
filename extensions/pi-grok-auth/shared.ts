export type AuthType = "oauth" | "api_key";

export type AuthMethod = {
	name?: string;
	loginLabel?: string;
	login?: unknown;
};

export type ProviderOption = {
	id: string;
	name: string;
	authType: AuthType;
	method?: AuthMethod;
	status?: { type: AuthType; source?: string };
};

export type ModelRuntimeLike = {
	getProviders: () => Array<{
		id: string;
		name: string;
		auth?: { oauth?: AuthMethod; apiKey?: AuthMethod };
	}>;
	getProvider?: (id: string) => { name?: string } | undefined;
	getProviderAuthStatus: (id: string) => {
		configured?: boolean;
		source?: string;
		label?: string;
	};
	isUsingOAuth?: (id: string) => boolean;
	listCredentials:
		| (() => Promise<Array<{ providerId: string; type: AuthType }>>)
		| (() => Array<{ providerId: string; type: AuthType }>);
	login: (
		providerId: string,
		method: AuthType,
		interaction: {
			signal?: AbortSignal;
			prompt: (prompt: AuthPrompt) => Promise<string>;
			notify: (event: AuthNotify) => void;
		},
		options?: { agentName?: string },
	) => Promise<unknown>;
	logout: (providerId: string) => Promise<void>;
	getAvailable?: () => Promise<unknown> | unknown;
	refresh?: (options?: unknown) => Promise<unknown>;
};

export type AuthPrompt = { signal?: AbortSignal } &
	(
		| { type: "text"; message: string; placeholder?: string }
		| { type: "secret"; message: string; placeholder?: string }
		| { type: "manual_code"; message: string; placeholder?: string }
		| { type: "select"; message: string; options: Array<{ id: string; label: string }> }
	);

export type AuthNotify =
	| { type: "auth_url"; url: string; instructions?: string }
	| { type: "device_code"; userCode?: string; verificationUri?: string; [key: string]: unknown }
	| { type: "info"; message: string; links?: unknown[] }
	| { type: "progress"; message: string };

export type ModelRegistryLike = {
	runtime?: ModelRuntimeLike;
	getProviderDisplayName?: (id: string) => string;
	refresh?: () => unknown;
};


export const AUTH_DIALOG_PREFIX = "__pi_grok_auth_dialog_v1__:";
export const AUTH_DIALOG_DONE_STATUS = "__pi_grok_auth_dialog_done__";
