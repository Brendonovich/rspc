import { getAbortController, getFetch } from "../../internals/fetchHelpers";
import { PromiseAndCancel, TRPCClientRuntime } from "../types";
import { HTTPHeaders } from "../../";

type ProcedureType = any; // TODO
type TRPCResponse = any; // TODO

export interface HTTPLinkOptions {
  url: string;
  /**
   * Add ponyfill for fetch
   */
  fetch?: typeof fetch;
  /**
   * Add ponyfill for AbortController
   */
  AbortController?: typeof AbortController | null;
  /**
   * Headers to be set on outgoing requests or a callback that of said headers
   * @link http://trpc.io/docs/v10/header
   */
  headers?: HTTPHeaders | (() => HTTPHeaders | Promise<HTTPHeaders>);
}

export interface ResolvedHTTPLinkOptions {
  url: string;
  fetch: typeof fetch;
  AbortController: typeof AbortController | null;
  /**
   * Headers to be set on outgoing request
   * @link http://trpc.io/docs/v10/header
   */
  headers: () => HTTPHeaders | Promise<HTTPHeaders>;
}

export function resolveHTTPLinkOptions(
  opts: HTTPLinkOptions
): ResolvedHTTPLinkOptions {
  const headers = opts.headers || (() => ({}));
  return {
    url: opts.url,
    fetch: getFetch(opts.fetch),
    AbortController: getAbortController(opts.AbortController),
    headers: typeof headers === "function" ? headers : () => headers,
  };
}

// TODO: This shouldn't be exported but it is for now
// https://github.com/trpc/trpc/pull/669
export function arrayToDict(array: unknown[]) {
  const dict: Record<number, unknown> = {};
  for (let index = 0; index < array.length; index++) {
    const element = array[index];
    dict[index] = element;
  }
  return dict;
}

const METHOD = {
  query: "GET",
  mutation: "POST",
} as const;

export interface HTTPResult {
  json: TRPCResponse;
  meta: {
    response: Response;
  };
}

type GetInputOptions = {
  runtime: TRPCClientRuntime;
} & ({ inputs: unknown[] } | { input: unknown });

// TODO: Shouldn't be exported
export function getInput(opts: { inputs: unknown[] } | { input: unknown }) {
  return "input" in opts
    ? opts.input // opts.runtime.transformer.serialize(opts.input)
    : arrayToDict(
        opts.inputs // .map((_input) => opts.runtime.transformer.serialize(_input))
      );
}

export type HTTPRequestOptions = ResolvedHTTPLinkOptions &
  GetInputOptions & {
    type: ProcedureType;
    path: string;
  };

export function getUrl(opts: HTTPRequestOptions) {
  let url = opts.url + "/" + opts.path;
  const queryParts: string[] = [];
  if ("inputs" in opts) {
    queryParts.push("batch=1");
  }
  if (opts.type === "query") {
    const input = getInput(opts);
    if (input !== undefined) {
      queryParts.push(`input=${encodeURIComponent(JSON.stringify(input))}`);
    }
  }
  if (queryParts.length) {
    url += "?" + queryParts.join("&");
  }
  return url;
}

type GetBodyOptions = { type: ProcedureType } & GetInputOptions;

export function getBody(opts: GetBodyOptions) {
  if (opts.type === "query") {
    return undefined;
  }
  const input = getInput(opts);
  return input !== undefined ? JSON.stringify(input) : undefined;
}

export function httpRequest(
  opts: HTTPRequestOptions
): PromiseAndCancel<HTTPResult> {
  const { type } = opts;
  const ac = opts.AbortController ? new opts.AbortController() : null;

  const promise = new Promise<HTTPResult>((resolve, reject) => {
    const url = getUrl(opts);
    const body = getBody(opts);

    const meta = {} as HTTPResult["meta"];
    Promise.resolve(opts.headers())
      .then((headers) => {
        if (type === "subscription") {
          throw new Error("Subscriptions should use wsLink");
        }
        return opts.fetch(url, {
          // @ts-expect-error: TODO
          method: METHOD[type],
          signal: ac?.signal,
          body: body,
          headers: {
            "content-type": "application/json",
            ...headers,
          },
        });
      })
      .then((_res) => {
        meta.response = _res;
        return _res.json();
      })
      .then((json) => {
        resolve({
          json,
          meta,
        });
      })
      .catch(reject);
  });
  const cancel = () => {
    ac?.abort();
  };
  return { promise, cancel };
}