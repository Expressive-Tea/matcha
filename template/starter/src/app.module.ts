import { Module } from '@green-tea/core';
import { HomeController } from './controllers/home.controller';

@Module({ mountpoint: '/', controllers: [HomeController] })
export class AppModule {}
