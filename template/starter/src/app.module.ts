import { Module } from '@green-tea/core';
import { HomeController } from './controllers/home.controller';

@Module({ controllers: [HomeController] })
export class AppModule {}
