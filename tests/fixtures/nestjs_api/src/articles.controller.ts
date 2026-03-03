import { Controller, Get, Post, Put, Delete, UseGuards, Param, Body } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';

@Controller('api/articles')
@UseGuards(AuthGuard('jwt'))
export class ArticlesController {
  @Get()
  findAll() {
    console.log("Listing all articles");
    return [];
  }

  @Get(':slug')
  findOne(@Param('slug') slug: string) {
    return {};
  }

  @Post()
  create(@Body() dto: any) {
    console.log("Creating article for:", dto.author.email);
    return {};
  }

  @Put(':slug')
  update(@Param('slug') slug: string, @Body() dto: any) {
    return {};
  }

  @Delete(':slug')
  remove(@Param('slug') slug: string) {
    console.log("Deleting article:", slug);
    return {};
  }
}
