// This file was generated by [rspc](https://github.com/oscartbeaumont/rspc). Do not edit this file manually.

export type Procedures = {
    queries: 
        { key: "users", input: never, result: User[] },
    mutations: 
        { key: "createUser", input: Input, result: User },
    subscriptions: never
};

export type Input = { email: string }

export type User = { id: number; email: string; name: string | null }